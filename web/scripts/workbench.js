// Phase γ (v0.6): shared client module for the live Workbench pages.
// Vanilla JS, no framework, no build step. Exposes window.vela.* helpers.

(function () {
  const base = window.location.origin;

  async function fetchFrontier() {
    const r = await fetch(`${base}/api/frontier`);
    if (!r.ok) throw new Error(`/api/frontier ${r.status}`);
    return r.json();
  }

  async function fetchFinding(id) {
    const r = await fetch(`${base}/api/findings/${encodeURIComponent(id)}`);
    if (!r.ok) throw new Error(`/api/findings/${id} ${r.status}`);
    return r.json();
  }

  async function fetchEvents(opts) {
    const params = new URLSearchParams();
    if (opts && opts.since) params.set('since', opts.since);
    if (opts && opts.limit) params.set('limit', String(opts.limit));
    const r = await fetch(`${base}/api/events?${params.toString()}`);
    if (!r.ok) throw new Error(`/api/events ${r.status}`);
    return r.json();
  }

  async function queueAction(kind, args) {
    const r = await fetch(`${base}/api/queue`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ kind, args }),
    });
    const data = await r.json();
    if (!r.ok) throw new Error(data.error || `queue ${r.status}`);
    return data;
  }

  function escapeHtml(s) {
    const div = document.createElement('div');
    div.textContent = s == null ? '' : String(s);
    return div.innerHTML;
  }

  function fmtDate(iso) {
    if (!iso) return '—';
    try { return new Date(iso).toISOString().slice(0, 10); }
    catch { return iso; }
  }

  function provenanceLink(prov) {
    if (!prov) return '';
    if (prov.doi) return `<a href="https://doi.org/${escapeHtml(prov.doi)}" target="_blank" rel="noopener">doi:${escapeHtml(prov.doi)}</a>`;
    if (prov.pmid) return `<a href="https://pubmed.ncbi.nlm.nih.gov/${escapeHtml(prov.pmid)}" target="_blank" rel="noopener">PMID:${escapeHtml(prov.pmid)}</a>`;
    if (prov.title) return escapeHtml(prov.title);
    return '';
  }

  window.vela = {
    base,
    fetchFrontier,
    fetchFinding,
    fetchEvents,
    queueAction,
    escapeHtml,
    fmtDate,
    provenanceLink,
  };
})();
