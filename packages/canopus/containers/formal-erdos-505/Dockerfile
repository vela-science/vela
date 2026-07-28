FROM ghcr.io/leanprover-community/mathlib4/lean@sha256:d64b66663cba8c517b2314d59bbde8864460ae2ef513b0e8cbca1278cd1229f6 AS builder

ARG SOURCE_COMMIT=c252a41054125b5fd9c8356e2137cd9b55337657
ARG SOURCE_TREE=a3eb8cb29f72a7ec1e7534955a5af0f6f72775c3
ARG SOURCE_FILE_SHA256=085b536fb121ff3ed0c13a390e0e3b72b3be7e4f5dec7a5702efd5884e8d93eb
ARG MATHLIB_COMMIT=a3a10db0e9d66acbebf76c5e6a135066525ac900

USER lean
WORKDIR /home/lean

RUN /home/lean/.elan/bin/elan toolchain install leanprover/lean4:v4.27.0

# Verify the exact upstream statement source, then discard the checkout. The
# runtime owns a reviewed copy of that statement and does not need the umbrella
# FormalConjecturesUtil import or its full-Mathlib cache closure.
RUN git init formal-source \
    && cd formal-source \
    && git remote add origin https://github.com/google-deepmind/formal-conjectures.git \
    && git fetch --depth 1 origin "$SOURCE_COMMIT" \
    && git checkout --detach FETCH_HEAD \
    && test "$(git rev-parse HEAD)" = "$SOURCE_COMMIT" \
    && test "$(git rev-parse HEAD^{tree})" = "$SOURCE_TREE" \
    && test "$(sha256sum FormalConjectures/ErdosProblems/505.lean | cut -d ' ' -f 1)" = "$SOURCE_FILE_SHA256" \
    && cd .. \
    && rm -rf formal-source

RUN git init mathlib \
    && cd mathlib \
    && git remote add origin https://github.com/leanprover-community/mathlib4.git \
    && git fetch --depth 1 origin "$MATHLIB_COMMIT" \
    && git checkout --detach FETCH_HEAD \
    && test "$(git rev-parse HEAD)" = "$MATHLIB_COMMIT"

WORKDIR /home/lean/mathlib
RUN /home/lean/.elan/bin/elan run leanprover/lean4:v4.27.0 lake exe cache get \
      Mathlib/Analysis/InnerProductSpace/PiL2.lean \
      Mathlib/Topology/MetricSpace/Bounded.lean
RUN printf '%s\n' \
      'import Mathlib.Analysis.InnerProductSpace.PiL2' \
      'import Mathlib.Topology.MetricSpace.Bounded' \
      'open Metric Set Classical' \
      'example (S : Set (EuclideanSpace ℝ (Fin 1)))' \
      '    (hS : Bornology.IsBounded S) (hd : 0 < diam S) :' \
      '    ∃ (F : Fin 2 → Set (EuclideanSpace ℝ (Fin 1))),' \
      '      S ⊆ ⋃ i, F i ∧ ∀ i, diam (F i) < diam S := by' \
      '  sorry' \
    | /home/lean/.elan/toolchains/leanprover--lean4---v4.27.0/bin/lake env lean --stdin
RUN rm -rf /home/lean/.cache /home/lean/mathlib/.git \
    && find /home/lean/mathlib/.lake/packages -type d -name .git -prune -exec rm -rf '{}' +

FROM ghcr.io/leanprover-community/mathlib4/lean@sha256:d64b66663cba8c517b2314d59bbde8864460ae2ef513b0e8cbca1278cd1229f6

LABEL org.opencontainers.image.source="https://github.com/vela-science/vela-research-harness"
LABEL org.opencontainers.image.description="Frozen minimal Lean verifier environment for formal:erdos-505-test-dim-one"
LABEL org.opencontainers.image.licenses="Apache-2.0 OR MIT"

USER lean
COPY --from=builder --chown=lean:lean \
  /home/lean/.elan/toolchains/leanprover--lean4---v4.27.0 \
  /home/lean/.elan/toolchains/leanprover--lean4---v4.27.0
COPY --from=builder --chown=lean:lean /home/lean/mathlib /home/lean/mathlib

WORKDIR /home/lean/mathlib
ENTRYPOINT []
