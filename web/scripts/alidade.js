// Alidade — the one blue index line on the left rim.
// Moves to the section currently in view as the reader scrolls.
// Honors prefers-reduced-motion by hiding the transition.
(function () {
  "use strict";

  var links = document.querySelectorAll("[data-alidade-target]");
  if (!links.length) return;

  var sections = {};
  links.forEach(function (link) {
    var id = link.getAttribute("data-alidade-target");
    var el = document.getElementById(id);
    if (el) sections[id] = { el: el, link: link };
  });

  var ids = Object.keys(sections);
  if (!ids.length) return;

  function setActive(id) {
    ids.forEach(function (other) {
      var entry = sections[other];
      if (other === id) entry.link.classList.add("site-rim__link--on");
      else entry.link.classList.remove("site-rim__link--on");
    });
  }

  // First link is active by default.
  setActive(ids[0]);

  if (!("IntersectionObserver" in window)) return;

  var observer = new IntersectionObserver(
    function (entries) {
      var best = null;
      entries.forEach(function (entry) {
        if (!entry.isIntersecting) return;
        if (!best || entry.intersectionRatio > best.intersectionRatio) best = entry;
      });
      if (!best) return;
      var id = best.target.id;
      if (id) setActive(id);
    },
    {
      root: null,
      rootMargin: "-30% 0px -60% 0px",
      threshold: [0, 0.25, 0.5, 0.75, 1],
    }
  );

  ids.forEach(function (id) { observer.observe(sections[id].el); });
})();
