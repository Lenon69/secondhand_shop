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
  let redirectUrl = event.detail.redirectUrl; // URL przekazany z backendu/loginSuccessDetails
  let pushUrl = event.detail.pushUrl || redirectUrl; // pushUrl przekazany lub taki sam jak redirectUrl

  if (isAuthenticated) {
    if (!redirectUrl) {
      // Jeśli nie ma konkretnego przekierowania, idź do moje-konto
      redirectUrl = "/htmx/moje-konto";
      pushUrl = "/moje-konto";
    }
  } else {
    // Dla wylogowania, zawsze na stronę logowania, chyba że specjalny redirect
    if (!redirectUrl) {
      redirectUrl = "/htmx/logowanie";
      pushUrl = "/logowanie";
    }
  }

  if (redirectUrl) {
    // Sprawdź, czy aktualna strona to już docelowy URL, aby uniknąć pętli
    if (window.location.pathname !== pushUrl) {
      console.log(
        "authChangedClient: Performing HTMX redirect to",
        redirectUrl,
        "pushing",
        pushUrl,
      );
      htmx.ajax("GET", redirectUrl, {
        target: "#content",
        swap: "innerHTML",
        pushUrl: pushUrl,
      });
    } else {
      console.log(
        "authChangedClient: Already on target page or no redirect needed.",
        pushUrl,
      );
      // Można rozważyć odświeżenie zawartości, jeśli strona ta sama, ale wymaga aktualizacji
      htmx.trigger(document.getElementById("content"), "reload-content");
    }
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
    // Powiadomienie o sukcesie jest już wywołane przez serwerowy trigger "showMessage"
    // Nie przekierowujemy od razu, pozwalamy na wyświetlenie komunikatu "showMessage"
    // Wysyłamy zdarzenie, które może być przechwycone przez Alpine.js lub inny centralny handler
    // aby zaktualizować stan i potencjalnie przekierować PO wyświetleniu komunikatu.
    setTimeout(() => {
      window.dispatchEvent(
        new CustomEvent("authChangedClient", {
          detail: {
            isAuthenticated: true,
            redirectUrl: "/htmx/moje-konto", // Domyślne przekierowanie po logowaniu
            pushUrl: "/moje-konto",
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

document.body.addEventListener("htmx:responseError", function (evt) {
  const xhr = evt.detail.xhr;
  if (xhr.status === 401) {
    console.warn("🔥 Otrzymano 401 Unauthorized – sesja mogła wygasnąć.");
    localStorage.removeItem("jwtToken"); // Wyczyść token na kliencie
    window.dispatchEvent(
      new CustomEvent("authChangedClient", {
        // Poinformuj Alpine.js i inne części o zmianie
        detail: {
          isAuthenticated: false,
          redirectUrl: "/htmx/logowanie", // Sugeruj przekierowanie na logowanie
          pushUrl: "/logowanie",
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
    // Nie przekierowujemy tutaj bezpośrednio, pozwalamy authChangedClient to obsłużyć
  }
  // Można tu dodać obsługę innych błędów, np. 403, 500 i wyświetlać odpowiednie komunikaty
});
