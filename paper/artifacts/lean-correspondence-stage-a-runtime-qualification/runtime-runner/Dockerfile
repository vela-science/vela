FROM scratch
ARG SOURCE_DATE_EPOCH
COPY runner /opt/vela/runner
COPY bridge /opt/vela/bridge
COPY provider-contract.json /opt/vela/provider-contract.json
COPY vendor/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
RUN --network=none [ "/opt/vela/runner", "--self-test" ]
RUN --network=none [ "/opt/vela/bridge", "--self-test" ]
USER 65532:65532
ENTRYPOINT ["/opt/vela/runner"]
CMD ["--run"]
