FROM --platform=linux/amd64 alpine@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce

RUN apk add --no-cache bash py3-pip python3 \
 && printf '%s\n' \
      'rfc8785==0.1.4 --hash=sha256:520d690b448ecf0703691c76e1a34a24ddcd4fc5bc41d589cb7c58ec651bcd48' \
      > /tmp/requirements.txt \
 && pip install --break-system-packages --require-hashes -r /tmp/requirements.txt \
 && rm /tmp/requirements.txt
COPY . /tests/
RUN find /tests -type d -exec chmod 0555 {} + \
 && find /tests -type f -exec chmod 0444 {} +
