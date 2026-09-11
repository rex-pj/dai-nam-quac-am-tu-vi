/* Tăng cường dần — trang phải chạy đầy đủ khi file này không tải được.
 *
 * Công cụ học thuật hay bị dùng trong môi trường lạ: máy thư viện, mạng yếu, trình đọc màn
 * hình, trình duyệt chặn script. Vì vậy mọi thứ ở đây chỉ THÊM tiện nghi, không có tính năng
 * nào chỉ tồn tại nhờ JavaScript:
 *   - Công tắc "Hiện dạng đầy đủ" là một <details>/checkbox hoạt động bằng CSS.
 *   - Tìm kiếm là <form method="get">, chạy khi không có JS.
 *   - Chip chế độ là <a> thật, có href.
 */
(function () {
  "use strict";

  /* ── Phím tắt ────────────────────────────────────────────────────────────
     "/" nhảy vào ô tìm, Esc xoá. Bỏ qua khi con trỏ đang ở trong một ô nhập,
     nếu không thì gõ chữ "/" trong truy vấn sẽ bị nuốt.                      */
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

  /* ── Chép tự dạng ────────────────────────────────────────────────────────
     Người dùng KHÔNG gõ được 𨰲, nên chép là thao tác chính chứ không phải phụ.
     Nút chỉ được thêm khi trình duyệt thật sự chép được — một nút bấm không có
     tác dụng còn tệ hơn không có nút.                                        */
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

  /* ── Chip chế độ tự sáng theo nội dung đang gõ ───────────────────────────
     Chỉ là gợi ý trực quan: giá trị thật vẫn do <select>/liên kết quyết định.
     Dải ký tự phải khớp `core::search::is_han_nom`; lệch nhau thì chip nói dối. */
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

  /* ── Công tắc "Hiện dạng đầy đủ" ─────────────────────────────────────────
     CSS đã làm phần hiện/ẩn; ở đây chỉ ghi nhớ lựa chọn trong phiên làm việc,
     để người đang tra nhiều mục không phải bấm lại ở từng trang.             */
  var toggle = document.querySelector("[data-expand-toggle]");
  var list = document.querySelector("[data-sub-list]");
  if (toggle && list) {
    var KEY = "dnqatv:hien-dang-day-du";
    var stored = null;
    try { stored = sessionStorage.getItem(KEY); } catch (e) { /* chế độ riêng tư */ }
    if (stored === "1") {
      toggle.checked = true;
      list.classList.add("show-expanded");
    }
    toggle.addEventListener("change", function () {
      list.classList.toggle("show-expanded", toggle.checked);
      try { sessionStorage.setItem(KEY, toggle.checked ? "1" : "0"); } catch (e) { /* bỏ qua */ }
    });
  }
})();
