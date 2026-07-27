# JavaScript reference emitter

`vela_emit.mjs` is a dependency-free implementation of the current producer
and verifier wire boundary. It does not import Vela or Canopus and cannot
construct Events or Decisions.

```bash
node clients/javascript/vela_emit.mjs submission \
  --draft submission-draft.json \
  --seed-file producer.seed.hex \
  --output submission.json

node clients/javascript/vela_emit.mjs verification \
  --draft verification-draft.json \
  --seed-file verifier.seed.hex \
  --output verification.json
```

Drafts contain the public object fields except `schema`, the content-addressed
ID, and `authentication`. The seed file contains one lowercase 32-byte Ed25519
seed and must have mode `0600` or stricter. The emitted canonical JSON is
accepted by the same Vela writer used for Canopus output.

The implementation exists to prove portability, not to provide key custody.
Production workbenches should use their platform signer and emit the same
closed objects.
