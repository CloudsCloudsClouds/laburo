// Laburo web UI: small, dependency-free progressive enhancement.
(function () {
  "use strict";

  var isTyping = function (target) {
    if (!target) return false;
    var tag = (target.tagName || "").toLowerCase();
    return tag === "input" || tag === "textarea" || tag === "select" || target.isContentEditable;
  };

  // Flash toast: dismiss + gentle auto-hide.
  var flash = document.getElementById("flash");
  if (flash) {
    var close = flash.querySelector(".flash-close");
    var hide = function () {
      flash.classList.add("flash-out");
      window.setTimeout(function () { flash.remove(); }, 320);
    };
    if (close) close.addEventListener("click", hide);
    window.setTimeout(hide, 7000);
  }

  // Copy IDs and links.
  document.querySelectorAll("[data-copy]").forEach(function (button) {
    button.addEventListener("click", function () {
      var value = button.getAttribute("data-copy") || "";
      var done = function () {
        var previous = button.textContent;
        button.textContent = "copied";
        button.classList.add("copied");
        window.setTimeout(function () {
          button.textContent = previous;
          button.classList.remove("copied");
        }, 1100);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(value).then(done, function () {});
      } else {
        var input = document.createElement("textarea");
        input.value = value;
        input.style.position = "fixed";
        input.style.opacity = "0";
        document.body.appendChild(input);
        input.select();
        try { document.execCommand("copy"); done(); } catch (e) {}
        document.body.removeChild(input);
      }
    });
  });

  // Confirm destructive submits.
  document.querySelectorAll("[data-confirm]").forEach(function (form) {
    form.addEventListener("submit", function (event) {
      if (!window.confirm(form.getAttribute("data-confirm"))) {
        event.preventDefault();
      }
    });
  });

  // Dirty-form warning for long forms.
  document.querySelectorAll("form[data-dirty]").forEach(function (form) {
    var dirty = false;
    var submitting = false;
    form.addEventListener("input", function () { dirty = true; });
    form.addEventListener("change", function () { dirty = true; });
    form.addEventListener("submit", function () { submitting = true; dirty = false; });
    window.addEventListener("beforeunload", function (event) {
      if (!dirty || submitting) return;
      event.preventDefault();
      event.returnValue = "";
      return "";
    });
  });

  // "/" focuses the search field, unless the user is already typing.
  document.addEventListener("keydown", function (event) {
    if (event.key === "/" && !isTyping(event.target) && !event.metaKey && !event.ctrlKey && !event.altKey) {
      var search = document.getElementById("search") || document.getElementById("global-search");
      if (search) {
        event.preventDefault();
        search.focus();
        search.select();
      }
    }
    if (event.key === "n" && !isTyping(event.target) && !event.metaKey && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      window.location.href = "/cases/new";
    }
  });

  // Filters that apply on change.
  document.querySelectorAll("[data-autosubmit]").forEach(function (element) {
    element.addEventListener("change", function () {
      var form = element.closest("form");
      if (form) form.submit();
    });
  });

  // Tag suggestions: click adds the tag to the comma-separated input.
  document.querySelectorAll("[data-add-tag]").forEach(function (button) {
    button.addEventListener("click", function () {
      var input = document.getElementById(button.getAttribute("data-add-tag") || "tags");
      if (!input) return;
      var tag = (button.getAttribute("data-tag") || "").trim();
      if (!tag) return;
      var current = input.value.split(",").map(function (item) { return item.trim(); }).filter(Boolean);
      if (current.indexOf(tag) === -1) current.push(tag);
      input.value = current.join(", ");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
  });

  // Select-all over a table of row checkboxes.
  document.querySelectorAll("[data-select-all]").forEach(function (master) {
    master.addEventListener("change", function () {
      var selector = master.getAttribute("data-select-all");
      document.querySelectorAll(selector).forEach(function (box) {
        box.checked = master.checked;
      });
      updateBulkCount();
    });
  });

  function selectedCases() {
    return Array.prototype.slice.call(document.querySelectorAll(".row-check:checked"));
  }

  function updateBulkCount() {
    var count = selectedCases().length;
    var label = document.getElementById("bulk-count");
    if (label) {
      label.textContent = count === 0 ? "No cases selected" : count + " case(s) selected";
    }
  }

  document.querySelectorAll(".row-check").forEach(function (box) {
    box.addEventListener("change", updateBulkCount);
  });

  var bulkForm = document.getElementById("bulk-form");
  if (bulkForm) {
    bulkForm.addEventListener("submit", function (event) {
      var ids = selectedCases().map(function (box) { return box.value; });
      var hidden = document.getElementById("bulk-ids");
      if (hidden) hidden.value = ids.join(",");
      if (ids.length === 0) {
        event.preventDefault();
        window.alert("Select at least one case first.");
        return;
      }
      var action = bulkForm.querySelector("[name=action]");
      if (action && !action.value) {
        event.preventDefault();
        action.focus();
      }
    });
  }

  // Report generation: build /reports/<kind>/<id> from the two inputs.
  var reportForm = document.getElementById("report-form");
  if (reportForm) {
    reportForm.addEventListener("submit", function (event) {
      event.preventDefault();
      var kind = reportForm.querySelector("[name=kind]").value;
      var idInput = reportForm.querySelector("[name=id]");
      var id = (idInput && idInput.value.trim()) || "all";
      window.location.href = "/reports/" + encodeURIComponent(kind) + "/" + encodeURIComponent(id);
    });
  }

  // File picker label.
  document.querySelectorAll("input[type=file]").forEach(function (input) {
    input.addEventListener("change", function () {
      var label = document.querySelector("[data-file-label='" + input.id + "']");
      if (label) {
        label.textContent = input.files && input.files.length ? input.files[0].name : "No file chosen";
      }
    });
  });

  // Link target helper: show the placeholder for the chosen kind.
  document.querySelectorAll("[data-link-kind]").forEach(function (select) {
    var update = function () {
      var input = document.getElementById(select.getAttribute("data-link-kind"));
      if (input) input.setAttribute("placeholder", select.value === "run" ? "RUN-…" : select.value === "bug" ? "BUG-…" : "TC-…");
    };
    select.addEventListener("change", update);
    update();
  });
})();
