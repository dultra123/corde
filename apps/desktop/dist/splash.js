// Page de démarrage de la coquille de bureau Corde.
//
// La coquille (Rust) pilote tout : elle démarre le serveur embarqué, puis navigue
// elle-même vers l'UI quand `/api/health` répond. Cette page se contente d'interroger
// la commande `boot_status` pour afficher l'erreur si le serveur n'a pas pu démarrer.
// Pas de module, pas de build : ES5 simple, `window.__TAURI__` vient de `withGlobalTauri`.
(function () {
  "use strict";

  var root = document.getElementById("splash");
  var tauri = window.__TAURI__;
  var invoke = tauri && tauri.core && tauri.core.invoke;

  function text(id, value) {
    var el = document.getElementById(id);
    if (el) el.textContent = value == null ? "" : String(value);
  }

  function render(status) {
    if (!status) return;
    if (status.version) text("version", "Corde " + status.version);
    if (status.phase === "failed") {
      root.setAttribute("data-phase", "failed");
      text("error-message", status.message || "Erreur inconnue.");
      text("log-path", status.log_path || "");
    } else {
      root.setAttribute("data-phase", "starting");
      if (status.phase === "ready") text("starting-hint", "Prêt — ouverture du tableau de bord…");
    }
  }

  function poll() {
    if (!invoke) return; // ouverte dans un navigateur ordinaire : page statique
    invoke("boot_status").then(
      function (status) {
        render(status);
        if (status && status.phase === "starting") setTimeout(poll, 250);
      },
      function () {
        setTimeout(poll, 1000);
      },
    );
  }

  var retry = document.getElementById("retry");
  if (retry) {
    retry.addEventListener("click", function () {
      if (!invoke) return;
      retry.disabled = true;
      invoke("restart_app").catch(function () {
        retry.disabled = false;
      });
    });
  }

  var copy = document.getElementById("copy-log");
  if (copy) {
    copy.addEventListener("click", function () {
      var path = document.getElementById("log-path").textContent || "";
      var done = function () {
        copy.textContent = "Chemin copié";
        setTimeout(function () {
          copy.textContent = "Copier le chemin du journal";
        }, 1800);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(path).then(done, function () {});
      }
    });
  }

  // Menu contextuel du navigateur inutile sur cette page (« Actualiser », « Inspecter »…).
  document.addEventListener("contextmenu", function (e) {
    var t = e.target;
    if (t && t.closest && t.closest(".message, .path")) return;
    e.preventDefault();
  });

  poll();
})();
