use super::*;

pub(super) struct Brc20Transferable {
  pub(super) destination: Address<NetworkChecked>,
  pub(super) inscription: Inscription,
  pub(super) commit_fee_rate: FeeRate,
  pub(super) mode: Mode,
  /// Do not back up recovery key
  pub(super) no_backup: bool,
}

impl Brc20Transferable {
  pub(crate) fn inscribe(&self, index: Arc<Index>) -> SubcommandResult {
    let address_ref: &[&Address<NetworkChecked>] = &[&self.destination];
    let address_option: Option<&[&Address<NetworkChecked>]> = Some(address_ref);
    let utxos = index
      .list_unspent(address_option)?
      .into_iter()
      .map(|utxo| {
        let outpoint = OutPoint::new(utxo.txid, utxo.vout);
        let amount = utxo.amount;

        (outpoint, amount)
      })
      .collect::<BTreeMap<OutPoint, Amount>>();

    let locked_utxos = index.list_lock_unspent()?;

    index.check_sync(&utxos)?;

    let runic_utxos = index.get_runic_outputs(&utxos.keys().cloned().collect::<Vec<OutPoint>>())?;

    let wallet_inscriptions = index.get_inscriptions(&utxos)?;

    // first index use extend change address
    let extend_change_address = index
      .get_extend_change_address()
      .expect("no extend change address exit in options");

    let commit_tx_change = [extend_change_address, self.destination.clone()];

    let (unsigned_commit_tx, reveal_tx, recovery_key_pair, total_fees) = self
      .create_batch_inscription_transactions(
        wallet_inscriptions,
        index.get_chain(),
        locked_utxos.clone(),
        runic_utxos,
        utxos.clone(),
        commit_tx_change,
      )?;

    let network = index.get_chain_network();
    if !self.no_backup {
      backup_recovery_key(index.as_ref(), recovery_key_pair, network)?;
    }

    Ok(Box::new(self.output_for_outside_sign(
      unsigned_commit_tx,
      reveal_tx,
      total_fees,
    )))
  }

  fn output_for_outside_sign(
    &self,
    unsigned_commit_tx: Transaction,
    signed_reveal_tx: Transaction,
    total_fees: u64,
  ) -> OutputForOutsideSign {
    let reveal_tx_id = signed_reveal_tx.txid();
    let inscriptions_output = InscriptionInfo {
      id: InscriptionId {
        txid: reveal_tx_id,
        index: 0,
      },
      location: SatPoint {
        outpoint: OutPoint {
          txid: reveal_tx_id,
          vout: 0,
        },
        offset: 0,
      },
    };

    OutputForOutsideSign {
      unsigned_commit_raw_tx_hex: unsigned_commit_tx.raw_hex(),
      signed_reveal_raw_tx_hex: signed_reveal_tx.raw_hex(),
      total_fees,
      inscription: inscriptions_output,
    }
  }

  pub(crate) fn create_batch_inscription_transactions(
    &self,
    wallet_inscriptions: BTreeMap<SatPoint, InscriptionId>,
    chain: Chain,
    locked_utxos: BTreeSet<OutPoint>,
    runic_utxos: BTreeSet<OutPoint>,
    mut utxos: BTreeMap<OutPoint, Amount>,
    change: [Address; 2],
  ) -> Result<(Transaction, Transaction, TweakedKeyPair, u64)> {
    let satpoint = {
      let inscribed_utxos = wallet_inscriptions
        .keys()
        .map(|satpoint| satpoint.outpoint)
        .collect::<BTreeSet<OutPoint>>();

      utxos
        .iter()
        .find(|(outpoint, amount)| {
          amount.to_sat() > 0
            && !inscribed_utxos.contains(outpoint)
            && !locked_utxos.contains(outpoint)
            && !runic_utxos.contains(outpoint)
        })
        .map(|(outpoint, _amount)| SatPoint {
          outpoint: *outpoint,
          offset: 0,
        })
        .ok_or_else(|| anyhow!("wallet contains no cardinal utxos"))?
    };

    for (inscribed_satpoint, inscription_id) in &wallet_inscriptions {
      if *inscribed_satpoint == satpoint {
        return Err(anyhow!("sat at {} already inscribed", satpoint));
      }

      if inscribed_satpoint.outpoint == satpoint.outpoint {
        return Err(anyhow!(
          "utxo {} already inscribed with inscription {inscription_id} on sat {inscribed_satpoint}",
          satpoint.outpoint,
        ));
      }
    }

    let secp256k1 = Secp256k1::new();
    let key_pair = UntweakedKeyPair::new(&secp256k1, &mut rand::thread_rng());
    let (public_key, _parity) = XOnlyPublicKey::from_keypair(&key_pair);

    let reveal_script = Inscription::append_batch_reveal_script(
      &vec![self.inscription.clone()],
      ScriptBuf::builder()
        .push_slice(public_key.serialize())
        .push_opcode(opcodes::all::OP_CHECKSIG),
    );

    let taproot_spend_info = TaprootBuilder::new()
      .add_leaf(0, reveal_script.clone())
      .expect("adding leaf should work")
      .finalize(&secp256k1, public_key)
      .expect("finalizing taproot builder should work");

    let control_block = taproot_spend_info
      .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
      .expect("should compute control block");

    let commit_tx_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), chain.network());

    let total_postage = match self.mode {
      Mode::SameSat => TARGET_POSTAGE,
      Mode::SharedOutput | Mode::SeparateOutputs => TARGET_POSTAGE * 1,
    };

    let mut reveal_inputs = vec![OutPoint::null()];
    let reveal_outputs = vec![TxOut {
      script_pubkey: self.destination.script_pubkey(),
      value: match self.mode {
        Mode::SeparateOutputs => TARGET_POSTAGE.to_sat(),
        Mode::SharedOutput | Mode::SameSat => total_postage.to_sat(),
      },
    }];

    let commit_input = 0;

    let (_, reveal_fee) = Self::build_reveal_transaction(
      &control_block,
      self.commit_fee_rate,
      reveal_inputs.clone(),
      commit_input,
      reveal_outputs.clone(),
      &reveal_script,
    );

    let unsigned_commit_tx = TransactionBuilder::new(
      satpoint,
      wallet_inscriptions,
      utxos.clone(),
      locked_utxos.clone(),
      runic_utxos,
      commit_tx_address.clone(),
      change,
      self.commit_fee_rate,
      Target::Value(reveal_fee + total_postage),
    )
    .build_transaction()?;

    let (vout, _commit_output) = unsigned_commit_tx
      .output
      .iter()
      .enumerate()
      .find(|(_vout, output)| output.script_pubkey == commit_tx_address.script_pubkey())
      .expect("should find sat commit/inscription output");

    reveal_inputs[commit_input] = OutPoint {
      txid: unsigned_commit_tx.txid(),
      vout: vout.try_into().unwrap(),
    };

    let (mut reveal_tx, _fee) = Self::build_reveal_transaction(
      &control_block,
      self.commit_fee_rate,
      reveal_inputs,
      commit_input,
      reveal_outputs.clone(),
      &reveal_script,
    );

    if reveal_tx.output[commit_input].value
      < reveal_tx.output[commit_input]
        .script_pubkey
        .dust_value()
        .to_sat()
    {
      bail!("commit transaction output would be dust");
    }

    let prevouts = vec![unsigned_commit_tx.output[vout].clone()];

    let mut sighash_cache = SighashCache::new(&mut reveal_tx);

    let sighash = sighash_cache
      .taproot_script_spend_signature_hash(
        commit_input,
        &Prevouts::All(&prevouts),
        TapLeafHash::from_script(&reveal_script, LeafVersion::TapScript),
        TapSighashType::Default,
      )
      .expect("signature hash should compute");

    let sig = secp256k1.sign_schnorr(
      &secp256k1::Message::from_slice(sighash.as_ref())
        .expect("should be cryptographically secure hash"),
      &key_pair,
    );

    let witness = sighash_cache
      .witness_mut(commit_input)
      .expect("getting mutable witness reference should work");

    witness.push(
      Signature {
        sig,
        hash_ty: TapSighashType::Default,
      }
      .to_vec(),
    );

    witness.push(reveal_script);
    witness.push(&control_block.serialize());

    let recovery_key_pair = key_pair.tap_tweak(&secp256k1, taproot_spend_info.merkle_root());

    let (x_only_pub_key, _parity) = recovery_key_pair.to_inner().x_only_public_key();
    assert_eq!(
      Address::p2tr_tweaked(
        TweakedPublicKey::dangerous_assume_tweaked(x_only_pub_key),
        chain.network(),
      ),
      commit_tx_address
    );

    let reveal_weight = reveal_tx.weight();

    if reveal_weight > bitcoin::Weight::from_wu(MAX_STANDARD_TX_WEIGHT.into()) {
      bail!(
        "reveal transaction weight greater than {MAX_STANDARD_TX_WEIGHT} (MAX_STANDARD_TX_WEIGHT): {reveal_weight}"
      );
    }

    utxos.insert(
      reveal_tx.input[commit_input].previous_output,
      Amount::from_sat(
        unsigned_commit_tx.output[reveal_tx.input[commit_input].previous_output.vout as usize]
          .value,
      ),
    );

    let total_fees = calculate_fee(&unsigned_commit_tx, &utxos) + calculate_fee(&reveal_tx, &utxos);

    Ok((unsigned_commit_tx, reveal_tx, recovery_key_pair, total_fees))
  }

  fn build_reveal_transaction(
    control_block: &ControlBlock,
    fee_rate: FeeRate,
    inputs: Vec<OutPoint>,
    commit_input_index: usize,
    outputs: Vec<TxOut>,
    script: &Script,
  ) -> (Transaction, Amount) {
    let reveal_tx = Transaction {
      input: inputs
        .iter()
        .map(|outpoint| TxIn {
          previous_output: *outpoint,
          script_sig: script::Builder::new().into_script(),
          witness: Witness::new(),
          sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        })
        .collect(),
      output: outputs,
      lock_time: LockTime::ZERO,
      version: 2,
    };

    let fee = {
      let mut reveal_tx = reveal_tx.clone();

      for (current_index, txin) in reveal_tx.input.iter_mut().enumerate() {
        // add dummy inscription witness for reveal input/commit output
        if current_index == commit_input_index {
          txin.witness.push(
            Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
              .unwrap()
              .to_vec(),
          );
          txin.witness.push(script);
          txin.witness.push(&control_block.serialize());
        } else {
          txin.witness = Witness::from_slice(&[&[0; SCHNORR_SIGNATURE_SIZE]]);
        }
      }

      fee_rate.fee(reveal_tx.vsize())
    };

    (reveal_tx, fee)
  }
}
