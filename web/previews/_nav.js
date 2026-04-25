// Shared workbench nav — the tick-limb rim.
// <script src="_nav.js" data-active="frontier"></script>

(function () {
  var me = document.currentScript;
  var active = (me && me.dataset.active) || '';
  var rim = document.getElementById('wb-rim');
  if (!rim) return;

  var links = [
    { id: 'frontier', href: 'frontier.html', label: '01 · Frontier' },
    { id: 'finding',  href: 'finding.html',  label: '02 · Finding'  },
    { id: 'terminal', href: 'terminal.html', label: '03 · Terminal' },
    { id: 'proof',    href: 'proof.html',    label: '04 · Proof'    },
  ];

  rim.innerHTML = '' +
    '<div class="wb-rim__mark">' +
      '<a href="frontier.html" aria-label="Vela">' +
        '<img src="../../assets/vela-logo-mark.svg" width="26" height="26" alt="">' +
      '</a>' +
    '</div>' +
    '<nav class="wb-rim__nav" aria-label="Workbench">' +
      links.map(function (l) {
        return '<a class="wb-rim__link' + (l.id === active ? ' wb-rim__link--on' : '') +
          '" href="' + l.href + '">' + l.label + '</a>';
      }).join('') +
    '</nav>' +
    '<div class="wb-rim__index">v0·2</div>';
})();
