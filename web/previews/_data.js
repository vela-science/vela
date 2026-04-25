// Sample data for the Vela Workbench. One frontier, eight findings,
// one focus finding with full evidence + history.

window.VELA_FRONTIER = {
  id: 'fr_bbb_alz',
  topic: 'Blood–brain barrier · Alzheimer\u2019s',
  version: 'v0.2',
  sealed_at: '2h ago',
  findings: 37,
  atoms: 142,
  links: 58,
  proof: 'stale',
  proof_hash: 'pk_44a9e1c',
};

window.VELA_FINDINGS = [
  { id: 'vf_2e1198da', cls: 'mechanism', state: 'replicated', conf: 0.84, links: 9, updated: '14 Jan',
    claim: 'LRP1 mediates BBB efflux of amyloid-\u03B2 in aged mice; knockout reduces clearance by ~40%.' },
  { id: 'vf_4f33be21', cls: 'method',    state: 'supported',  conf: 0.62, links: 4, updated: '12 Jan',
    claim: 'Brain:plasma ratio at a single timepoint is an unreliable proxy for sustained CNS exposure.' },
  { id: 'vf_9a3c21e8', cls: 'exposure',  state: 'contested',  conf: 0.71, links: 7, updated: '09 Jan',
    claim: 'TfR-targeted bispecific increases apparent CNS exposure in mice under declared dose and assay.' },
  { id: 'vf_6d8890fc', cls: 'exposure',  state: 'proposed',   conf: 0.55, links: 1, updated: '07 Jan',
    claim: 'Shuttle-mediated brain delivery of enzyme payloads exceeds IgG by >10\u00D7 in non-human primates.' },
  { id: 'vf_1b40ff3a', cls: 'mechanism', state: 'supported',  conf: 0.58, links: 6, updated: '06 Jan',
    claim: 'RAGE mediates A\u03B2 influx across the BBB, opposing LRP1-mediated efflux.' },
  { id: 'vf_7b4120af', cls: 'efficacy',  state: 'gap',        conf: 0.18, links: 2, updated: '03 Jan',
    claim: 'TfR-bispecific reduces A\u03B2 plaque load at clinically meaningful doses in human trials.' },
  { id: 'vf_a82c115d', cls: 'method',    state: 'supported',  conf: 0.49, links: 3, updated: '02 Jan',
    claim: 'Plasma A\u03B240/42 ratio drops before detectable cognitive decline by 6\u201312 months.' },
  { id: 'vf_c19833ea', cls: 'mechanism', state: 'replicated', conf: 0.66, links: 5, updated: '31 Dec',
    claim: 'Pericyte loss precedes BBB breakdown in aged APOE4 carriers.' },
];

// Engraved, muted — never neon.
window.VELA_STATE = window.VELA_STATE_COLOR = {
  replicated: { fg: '#3F6B4E', label: 'replicated' },
  supported:  { fg: '#1B1F27', label: 'supported'  },
  contested:  { fg: '#8A6A1F', label: 'contested'  },
  gap:        { fg: '#7A6F5C', label: 'gap'        },
  proposed:   { fg: '#4F5A7A', label: 'proposed'   },
  retracted:  { fg: '#8A3A3A', label: 'retracted'  },
};

window.VELA_FOCUS = {
  id: 'vf_9a3c21e8',
  cls: 'exposure',
  state: 'contested',
  conf: 0.71,
  updated: '09 Jan 2026',
  reviewer: 'demo',
  claim: 'TfR-targeted bispecific increases apparent CNS exposure in mice under declared dose, assay, and endpoint.',
  note: 'Held open; evidence supports and contradicts under different affinity regimes. Keep conditions explicit.',
  conditions: [
    { k: 'species',    v: 'Mus musculus' },
    { k: 'model',      v: 'aged Tg2576' },
    { k: 'assay',      v: 'LC-MS/MS plasma\u00B7brain' },
    { k: 'endpoint',   v: 'apparent CNS exposure' },
    { k: 'payload',    v: 'anti-A\u03B2 Fab' },
    { k: 'comparator', v: 'isotype control' },
  ],
  evidence: [
    { id: 'ev_1a2', src: 'Yu 2011 \u00B7 Sci Transl Med', loc: 'Fig. 3B',        stance: 'supports',
      q: 'TfR/BACE1 bispecific increased brain:plasma ratio 4.9\u00D7 over isotype control in aged mice (n = 8, p < 0.001).' },
    { id: 'ev_1a3', src: 'Couch 2013',                    loc: 'Table 2',        stance: 'supports',
      q: 'Replication in an independent cohort; similar affinity-dependent BBB transit observed.' },
    { id: 'ev_1a4', src: 'Bien-Ly 2014',                  loc: '\u00A7Results p.234', stance: 'contradicts',
      q: 'High-affinity variants showed reduced brain uptake and accelerated TfR degradation.' },
    { id: 'ev_1a5', src: 'PDB 4X3K',                      loc: 'structural',     stance: 'supports',
      q: 'Low-affinity TfR binding preserves receptor recycling \u2014 consistent with the observed exposure effect.' },
  ],
  history: [
    { at: '09 Jan', who: 'demo',  what: 'state changed', detail: 'supported \u2192 contested' },
    { at: '09 Jan', who: 'demo',  what: 'evidence added', detail: 'ev_1a4 \u00B7 Bien-Ly 2014 (contradicts)' },
    { at: '03 Jan', who: 'agent', what: 'created',        detail: 'from 4 papers \u00B7 conf 0.71' },
  ],
};
