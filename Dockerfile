# Use a base image compatible with your Linux executable
FROM ubuntu:latest

# Copy the ord executable into the container
EXPOSE 80
COPY ord /usr/local/bin/ord

RUN apt update && apt install -y libssl-dev libc6 && apt clean

# Set executable permissions for ord
RUN chmod +x /usr/local/bin/ord

# Set the command to run when the container starts
CMD ["/bin/sh", "-c", "/usr/local/bin/ord $EXTRA_PARAMS server --http"]