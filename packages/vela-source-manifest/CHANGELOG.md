# Changelog

## 2.0.0

**Breaking: the `home` declaration field is removed. Use `homepage`.**

`home` was carried as a legacy alias for `homepage` because, as the schema put
it, "the Erdős generator's passthrough list named it". That generator's
repository writes `homepage`, and so does every other declaration reachable
today — `vela-science/math`, the four archived topic repositories, and the
`vela-web` vendored copy. The alias had no producer.

It also had no reader. Nothing normalized `home` into `homepage`: it was listed
in the resolver's `PASSTHROUGH` tuple and the verifier's `VERBATIM` tuple, so a
declaration writing it got a lock that also wrote it, and every consumer looking
for a landing page found nothing. A second accepted spelling that no code
resolves is a way to be silently ignored, not a convenience.

This is a major version because both schemas set `additionalProperties: false`.
A declaration or lock still carrying `home` now fails validation rather than
being accepted and dropped. Rename the field; nothing else changes.

## 1.0.0

First released version.
