// Plik app.js
document.body.addEventListener("htmx:configRequest", (event) => {
  if (!event.detail || !event.detail.headers) return;
  const guestCartId = localStorage.getItem("guestCartId");
  if (guestCartId) event.detail.headers["X-Guest-Cart-Id"] = guestCartId;
  const jwtToken = localStorage.getItem("jwtToken");
  if (jwtToken) event.detail.headers["Authorization"] = "Bearer " + jwtToken;
});

document.body.addEventListener("updateCartCount", (htmxEvent) => {
  if (!htmxEvent.detail) return;
  document.body.dispatchEvent(
    new CustomEvent("js-update-cart", {
      detail: htmxEvent.detail,
      bubbles: true,
    }),
  );
  if (typeof htmxEvent.detail.newCartTotalPrice !== "undefined") {
    const el = document.getElementById("cart-subtotal-price");
    if (el)
      el.innerHTML =
        (parseInt(htmxEvent.detail.newCartTotalPrice) / 100)
          .toFixed(2)
          .replace(".", ",") + " zł";
  }
});

document.body.addEventListener("htmx:afterSwap", function (event) {
  if (
    event.detail.target.id === "content" ||
    event.detail.target.closest("#content")
  ) {
    if (
      !window.location.pathname.endsWith("/logowanie") &&
      !window.location.pathname.endsWith("/rejestracja")
    ) {
      const loginMessages = document.getElementById("login-messages");
      if (loginMessages) loginMessages.innerHTML = "";
      const registrationMessages = document.getElementById(
        "registration-messages",
      );
      if (registrationMessages) registrationMessages.innerHTML = "";
    }
    window.scrollTo({ top: 0, behavior: "smooth" });
  }
});

// Centralny listener do obsługi zmian autoryzacji i przekierowań
document.addEventListener("authChangedClient", (event) => {
  console.log(
    "authChangedClient: isAuthenticated:",
    event.detail.isAuthenticated,
    "redirectUrl:",
    event.detail.redirectUrl,
    "current window.location.pathname:",
    window.location.pathname,
  );

  const isAuthenticated = event.detail.isAuthenticated;
  let redirectUrl = event.detail.redirectUrl;
  let pushUrl = event.detail.pushUrl || redirectUrl;

  if (isAuthenticated) {
    if (!redirectUrl) {
      redirectUrl = "/htmx/moje-konto";
      pushUrl = "/moje-konto";
    }
  } else {
    // Dla wylogowania lub błędu 401, zawsze na stronę logowania, chyba że specjalny redirect
    if (!redirectUrl) {
      redirectUrl = "/htmx/logowanie";
      pushUrl = "/logowanie";
    }
  }

  if (redirectUrl) {
    // Sprawdź, czy aktualna ścieżka (bez części /htmx/) to już docelowy URL
    // lub czy docelowy URL to ten sam, który spowodował 401 (aby uniknąć pętli, jeśli serwer źle skonfigurowany)
    const currentCleanPath = window.location.pathname.replace(/^\/htmx/, "");
    const targetCleanPushUrl = pushUrl ? pushUrl.replace(/^\/htmx/, "") : "";

    if (
      currentCleanPath !== targetCleanPushUrl ||
      window.location.pathname === "/" ||
      event.detail.forceRedirect
    ) {
      // Dodano forceRedirect
      console.log(
        "authChangedClient: Performing HTMX redirect to",
        redirectUrl,
        "pushing",
        pushUrl,
      );
      if (window.htmx) {
        // Upewnij się, że htmx jest dostępne
        htmx.ajax("GET", redirectUrl, {
          target: "#content",
          swap: "innerHTML",
          pushUrl: pushUrl,
        });
      } else {
        console.error("HTMX not available for redirection.");
      }
    } else {
      console.log(
        "authChangedClient: Already on target page or redirect loop avoided. Current:",
        window.location.pathname,
        "Target pushUrl:",
        pushUrl,
      );
      // Można rozważyć odświeżenie zawartości, jeśli strona ta sama, ale wymaga aktualizacji
      htmx.trigger(document.getElementById("content"), "reload-content");
      // lub wymusić, jeśli to np. błąd 401 na stronie moje-konto
      if (
        xhr &&
        xhr.status === 401 &&
        window.location.pathname.includes("/moje-konto")
      ) {
        // Jeśli dostaliśmy 401 będąc na /moje-konto, to chcemy przekierować na logowanie.
        // Ten warunek może być już obsłużony przez logikę powyżej.
      }
    }
  } else {
    console.log("authChangedClient: No redirectUrl specified.");
  }
});

document.body.addEventListener("authChangedFromBackend", function (evt) {
  if (evt.detail && typeof evt.detail.isAuthenticated !== "undefined") {
    if (evt.detail.token) {
      localStorage.setItem("jwtToken", evt.detail.token);
    } else if (!evt.detail.isAuthenticated) {
      localStorage.removeItem("jwtToken");
    }
    // Przekazujemy informację o przekierowaniu do centralnego listenera
    window.dispatchEvent(
      new CustomEvent("authChangedClient", {
        detail: {
          isAuthenticated: evt.detail.isAuthenticated,
          redirectUrl: evt.detail.redirectUrl, // Przekaż redirectUrl
          pushUrl: evt.detail.pushUrl, // Przekaż pushUrl
        },
      }),
    );
  }
});

document.body.addEventListener("loginSuccessDetails", function (evt) {
  console.log("loginSuccessDetails: Detail:", evt.detail);
  if (evt.detail && evt.detail.token) {
    localStorage.setItem("jwtToken", evt.detail.token);
    setTimeout(() => {
      window.dispatchEvent(
        new CustomEvent("authChangedClient", {
          detail: {
            isAuthenticated: true,
            redirectUrl: "/htmx/moje-konto",
            pushUrl: "/moje-konto",
            forceRedirect: true,
          },
        }),
      );
    }, 700); // Opóźnienie na wyświetlenie komunikatu
  } else {
    console.error(
      "[App.js] loginSuccessDetails event, but NO TOKEN:",
      evt.detail,
    );
    window.dispatchEvent(
      new CustomEvent("showMessage", {
        detail: {
          message: "Błąd logowania: brak tokenu (klient).",
          type: "error",
        },
      }),
    );
  }
});

document.body.addEventListener("registrationComplete", function (evt) {
  console.log(
    '<<<<< [App.js] "registrationComplete" EVENT RECEIVED >>>>>. Detail:',
    JSON.stringify(evt.detail),
  );
  const form = document.getElementById("registration-form");
  if (form && form.reset) {
    form.reset();
  }
  setTimeout(() => {
    if (window.htmx) {
      htmx.ajax("GET", "/htmx/logowanie", {
        // Przekierowanie na logowanie po rejestracji
        target: "#content",
        swap: "innerHTML",
        pushUrl: "/logowanie",
      });
    }
  }, 1500);
});

document.body.addEventListener("htmx:afterOnLoad", function (evt) {
  const response = evt.detail.xhr.responseText;
  try {
    const json = JSON.parse(response);
    if (json.showMessage) {
      window.dispatchEvent(
        new CustomEvent("showMessage", {
          detail: {
            message: json.showMessage.message,
            type: json.showMessage.type || "info",
          },
        }),
      );
    }
  } catch (_) {
    // Niepoprawny JSON – ignorujemy
  }
});

// Listener htmx:responseError (Twój kod, lekko rozszerzony o console.log dla pewności)
document.body.addEventListener("htmx:responseError", function (evt) {
  const xhr = evt.detail.xhr;
  if (xhr.status === 401) {
    console.warn(
      "🔥 Otrzymano 401 Unauthorized – sesja mogła wygasnąć. Usuwam token.",
    );
    localStorage.removeItem("jwtToken"); // Wyczyść token na kliencie

    console.log("Token JWT usunięty z localStorage."); // Dodatkowy log

    window.dispatchEvent(
      new CustomEvent("authChangedClient", {
        detail: {
          isAuthenticated: false,
          redirectUrl: "/htmx/logowanie", // Sugeruj przekierowanie na logowanie
          pushUrl: "/logowanie",
          forceRedirect: true, // Dodaj flagę, aby wymusić przekierowanie nawet jeśli ścieżki wydają się podobne
        },
      }),
    );
    window.dispatchEvent(
      new CustomEvent("showMessage", {
        detail: {
          message:
            "Twoja sesja wygasła lub nie masz uprawnień. Zaloguj się ponownie.",
          type: "warning",
        },
      }),
    );
  }
});
