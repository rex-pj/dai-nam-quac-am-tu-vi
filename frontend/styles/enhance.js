/* Progressive enhancement — the page must work in full when this file fails to load.
 *
 * A scholarly tool gets used in strange conditions: a library machine, a weak connection, a
 * screen reader, a browser that blocks scripts. So everything here only ADDS comfort, and no
 * feature exists that lives on JavaScript alone:
 *   - The "Hiện dạng đầy đủ" switch is a <details>/checkbox driven by CSS.
 *   - Search is a real <form method="get">, and runs without JS.
 *   - The mode chips are real <a> elements, with an href.
 */
(function () {
  "use strict";

  /* ── Keyboard shortcuts ──────────────────────────────────────────────────
     "/" jumps to the search box, Esc clears it. Skipped while the caret sits
     in a field, or a "/" typed inside a query would be swallowed.            */
  document.addEventListener("keydown", function (e) {
    var tag = (e.target && e.target.tagName) || "";
    var typing = tag === "INPUT" || tag === "TEXTAREA" || e.target.isContentEditable;

    if (e.key === "/" && !typing) {
      var box = document.querySelector("[data-search-input]");
      if (box) {
        e.preventDefault();
        box.focus();
        box.select();
      }
    }
    if (e.key === "Escape" && typing && e.target.value) {
      e.target.value = "";
    }
  });

  /* ── Copying a glyph ─────────────────────────────────────────────────────
     A reader CANNOT type 𨰲, so copying is the primary action, not a nicety.
     The button appears only where the browser can really copy — a button that
     does nothing is worse than no button at all.                             */
  if (navigator.clipboard && window.isSecureContext) {
    document.querySelectorAll("[data-copy]").forEach(function (btn) {
      btn.hidden = false;
      btn.addEventListener("click", function () {
        navigator.clipboard.writeText(btn.getAttribute("data-copy")).then(
          function () {
            var before = btn.textContent;
            btn.textContent = "đã chép";
            setTimeout(function () { btn.textContent = before; }, 1400);
          },
          function () { btn.textContent = "không chép được"; }
        );
      });
    });
  }

  /* ── The mode chip lights up to follow what is being typed ───────────────
     A visual hint only: the <select>/link still decides the value that is sent.
     The ranges must match `core::search::is_han_nom`; drift and the chip lies. */
  var input = document.querySelector("[data-search-input]");
  var autoChip = document.querySelector('[data-mode-chip="auto"]');
  if (input && autoChip) {
    var hanNom = /[㐀-䶿一-鿿豈-﫿]|[\uD840-\uD87F][\uDC00-\uDFFF]|[\uDB80-\uDBBF][\uDC00-\uDFFF]/;
    var update = function () {
      var v = input.value;
      var guess = v.length === 0 ? "auto" : hanNom.test(v) ? "han-nom" : "quoc-ngu";
      document.querySelectorAll("[data-mode-chip]").forEach(function (c) {
        c.classList.toggle("is-guess", c.getAttribute("data-mode-chip") === guess && v.length > 0);
      });
    };
    input.addEventListener("input", update);
    update();
  }

  /* ── The "Hiện dạng đầy đủ" switch ───────────────────────────────────────
     CSS already does the showing and hiding; this only remembers the choice for
     the session, so that reading many entries does not mean clicking on each. */
  var toggle = document.querySelector("[data-expand-toggle]");
  var list = document.querySelector("[data-sub-list]");
  if (toggle && list) {
    var KEY = "dnqatv:hien-dang-day-du";
    var stored = null;
    try { stored = sessionStorage.getItem(KEY); } catch (e) { /* Private mode. */ }
    if (stored === "1") {
      toggle.checked = true;
      list.classList.add("show-expanded");
    }
    toggle.addEventListener("change", function () {
      list.classList.toggle("show-expanded", toggle.checked);
      try { sessionStorage.setItem(KEY, toggle.checked ? "1" : "0"); } catch (e) { /* Ignored. */ }
    });
  }
})();
