# Action-complete Frontier campaign baseline

This artifact freezes the read-only source state for the next Vela campaign.
It changes no Frontier, Proposal, Event, Standing, projection, or deployment.

The canonical document is [`baseline.v1.json`](baseline.v1.json), rooted at:

```text
sha256:46f931b202618ef6437a23f0c49f9172cafa739c1b1b69465f5171f1caa39a4c
```

It binds:

- Vela `0.963.0`, source commit `efad27956dc162d1955dad488bbb5830a311ffa9`,
  and exact release-binary bytes;
- Harbor `0.20.0` and the current benchmark implementation roots;
- all four clean, strict-replaying Frontier heads;
- Observatory `0.430.0` at projection root
  `sha256:8bc68a34296b7e33bee7ca2321333bf84ea9d6b96867b55dd2c64ff85394917e`;
- the exact Erdős `erdos:1056` Target packet and inclusive next range
  `10430601..10430800`; and
- explicit no-Target results for Formal, Quantum, and Sidon.

The benchmark contract freezes matched `git-files` and `vela-guided` arms,
five heterogeneous task classes, Harbor custody, a two-run instrumentation
pilot with no claim credit, a power-derived confirmatory design, and the
ETY/VPAC/FIE/CPI/correction-resilience metrics.

This is a source-state baseline, not a model-output record. It earns no
performance, productivity, adoption, independence, interoperability, or
scientific claim. The controlled correction task remains a closed-ground-truth
product benchmark and cannot substitute for a real correction with downstream
Frontier topology. No agent or verifier may perform a scientific Decision.
