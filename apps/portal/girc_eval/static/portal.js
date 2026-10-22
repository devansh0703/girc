/* The portal's client-side behaviour.
 *
 * The forms submit over fetch and report the result in place, because the
 * portal is a working console and a full page reload for a one-line
 * confirmation loses the reader's place.
 */

(function () {
  "use strict";

  function message(text, kind) {
    var box = document.createElement("div");
    box.className = "notice" + (kind === "error" ? " error" : "");
    box.textContent = text;
    return box;
  }

  // Every form that declares itself as JSON submits over fetch.
  document.querySelectorAll("form[data-json]").forEach(function (form) {
    form.addEventListener("submit", function (event) {
      event.preventDefault();

      var data = new FormData(form);
      var previous = form.parentNode.querySelector(":scope > .notice[data-result]");
      if (previous) {
        previous.remove();
      }

      fetch(form.action, {
        method: "POST",
        body: data,
        headers: { Accept: "application/json" },
      })
        .then(function (response) {
          return response.json().then(function (body) {
            return { ok: response.ok, body: body };
          });
        })
        .then(function (result) {
          if (result.ok) {
            var notice = message(
              result.body.status === "registered"
                ? "Registered. Redirecting to the dashboard…"
                : "Done: " + (result.body.status || "ok") + ".",
              "ok"
            );
            notice.dataset.result = "1";
            form.parentNode.insertBefore(notice, form);
            if (result.body.status === "registered") {
              window.location.href = "/dashboard";
              return;
            }
            // The state has changed, so the page is reloaded to show it.
            window.setTimeout(function () {
              window.location.reload();
            }, 600);
          } else {
            var detail =
              typeof result.body.detail === "string"
                ? result.body.detail
                : "The request was refused.";
            var notice = message(detail, "error");
            notice.dataset.result = "1";
            form.parentNode.insertBefore(notice, form);
          }
        })
        .catch(function (error) {
          var notice = message("The request could not be sent: " + error, "error");
          notice.dataset.result = "1";
          form.parentNode.insertBefore(notice, form);
        });
    });
  });

  // A download request returns a signed link, which is then followed.
  document.querySelectorAll("[data-download]").forEach(function (button) {
    button.addEventListener("click", function () {
      var slug = button.getAttribute("data-download");
      button.disabled = true;
      button.textContent = "Preparing…";

      fetch("/artifacts/" + slug + "/download-token", {
        headers: { Accept: "application/json" },
      })
        .then(function (response) {
          return response.json().then(function (body) {
            return { ok: response.ok, body: body };
          });
        })
        .then(function (result) {
          if (!result.ok) {
            var detail =
              typeof result.body.detail === "string"
                ? result.body.detail
                : "The download was refused.";
            var notice = message(detail, "error");
            notice.dataset.result = "1";
            button.parentNode.parentNode.parentNode.insertBefore(
              notice,
              button.parentNode.parentNode.parentNode.firstChild
            );
            button.disabled = false;
            button.textContent = "Download";
            return;
          }
          var link = document.createElement("a");
          link.href = "/download?token=" + encodeURIComponent(result.body.token);
          link.textContent = result.body.filename || "download";
          link.className = "btn small";
          window.location.href = link.href;
          button.disabled = false;
          button.textContent = "Download";
        })
        .catch(function (error) {
          button.disabled = false;
          button.textContent = "Download";
          console.error(error);
        });
    });
  });
})();
