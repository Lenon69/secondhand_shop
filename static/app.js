// Plik app.js - UPROSZCZONA WERSJA STEROWANA ZDARZENIAMI Z SERWERA
// Listener htmx:configRequest (tylko dla globalnych nagłówków - BEZ ZMIAN)
document.body.addEventListener("htmx:configRequest", (event) => {
  if (!event.detail || !event.detail.headers) return;

  // Dodawanie X-Guest-Cart-Id
  const guestCartId = localStorage.getItem("guestCartId");
  if (guestCartId) event.detail.headers["X-Guest-Cart-Id"] = guestCartId;

  // Dodawanie tokenu JWT
  const jwtToken = localStorage.getItem("jwtToken");
  if (jwtToken) event.detail.headers["Authorization"] = "Bearer " + jwtToken;
});

// Listener updateCartCount (dla koszyka - BEZ ZMIAN)
document.body.addEventListener("updateCartCount", (htmxEvent) => {
  if (!htmxEvent.detail) return;

  // Emitowanie zdarzenia "js-update-cart"
  document.body.dispatchEvent(
    new CustomEvent("js-update-cart", {
      detail: htmxEvent.detail,
      bubbles: true,
    }),
  );

  // Aktualizacja ceny w koszyku
  if (typeof htmxEvent.detail.newCartTotalPrice !== "undefined") {
    const el = document.getElementById("cart-subtotal-price");
    if (el)
      el.innerHTML =
        (parseInt(htmxEvent.detail.newCartTotalPrice) / 100)
          .toFixed(2)
          .replace(".", ",") + " zł";
  }
});

// Listener htmx:afterSwap (dla przewijania i czyszczenia - BEZ ZMIAN)
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

// Listener authChangedFromBackend (dla stanu Alpine - BEZ ZMIAN)
document.body.addEventListener("authChangedFromBackend", function (evt) {
  if (evt.detail && typeof evt.detail.isAuthenticated !== "undefined") {
    if (evt.detail.token) localStorage.setItem("jwtToken", evt.detail.token);
    else if (!evt.detail.isAuthenticated) localStorage.removeItem("jwtToken");
    window.dispatchEvent(
      new CustomEvent("authChangedClient", {
        detail: { isAuthenticated: evt.detail.isAuthenticated },
      }),
    );
    if (evt.detail.isAuthenticated && evt.detail.redirectUrl) {
      const pushUrl = evt.detail.pushUrl || evt.detail.redirectUrl;
      htmx.ajax("GET", evt.detail.redirectUrl, {
        target: "#content",
        swap: "innerHTML",
        pushUrl: pushUrl,
      });
    }
  }
});

// Listener dla "loginSuccessDetails" (z HX-Trigger od serwera)
document.body.addEventListener("loginSuccessDetails", function (evt) {
  console.log("Detail:", evt.detail);
  if (evt.detail && evt.detail.token) {
    localStorage.setItem("jwtToken", evt.detail.token);
    window.dispatchEvent(
      new CustomEvent("authChangedClient", {
        detail: { isAuthenticated: true },
      }),
    );
    // Powiadomienie o sukcesie jest już wywołane przez serwerowy trigger "showMessage"
    // Przekierowanie po krótkim opóźnieniu, aby użytkownik zobaczył powiadomienie
    setTimeout(() => {
      if (window.htmx) {
        window.htmx.ajax("GET", "/htmx/moje-konto", {
          target: "#content",
          swap: "innerHTML",
          pushUrl: "/moje-konto",
        });
      }
    }, 700); // Krótsze opóźnienie
  } else {
    console.error(
      "[App.js] loginSuccessDetails event, but NO TOKEN:",
      evt.detail,
    );
    // To zdarzenie nie powinno być wywołane przez serwer, jeśli nie ma tokenu.
    //
    // Jeśli jednak, to pokażemy błąd.
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

// Listener dla "registrationComplete" (z HX-Trigger od serwera po udanej rejestracji)
document.body.addEventListener("registrationComplete", function (evt) {
  console.log(
    '<<<<< [App.js] "registrationComplete" EVENT RECEIVED >>>>>. Detail:',
    JSON.stringify(evt.detail),
  );
  const form = document.getElementById("registration-form");
  if (form && form.reset) {
    form.reset();
  }
  // Przekierowanie na stronę "Moje Konto" po udanej rejestracji i krótkim opóźnieniu
  setTimeout(() => {
    if (window.htmx) {
      window.htmx.ajax("GET", "/htmx/logowanie", {
        target: "#content",
        swap: "innerHTML",
        pushUrl: "/logowanie",
      });
    }
  }, 1500);
});

function getJwtToken() {
  return localStorage.getItem("jwtToken");
}

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
    console.warn("🔥 Otrzymano 401 Unauthorized – przekierowanie do logowania");
    window.dispatchEvent(
      new CustomEvent("showMessage", {
        detail: {
          message: "Twoja sesja wygasła. Zaloguj się ponownie.",
          type: "warning",
        },
      }),
    );

    // Przekieruj po krótkim czasie
    setTimeout(() => {
      window.location.href = "/logowanie";
    }, 1000);
  }
});

window.dispatchEvent(
  new CustomEvent("authChangedClient", {
    detail: { isAuthenticated: true },
  }),
);

window.dispatchEvent(new CustomEvent("logoutClient"));

document
  .querySelector('a[href="/moje-konto"]')
  .addEventListener("click", function (event) {
    event.preventDefault();
    htmx.ajax("GET", "/htmx/moje-konto", {
      headers: {
        Authorization: "Bearer " + getJwtToken(), // Funkcja, która pobiera JWT z pamięci
      },
      target: "#content",
      swap: "innerHTML",
    });
  });

document.addEventListener("authChangedClient", (event) => {
  console.log("Status autoryzacji:", event.detail.isAuthenticated);
  if (event.detail.isAuthenticated) {
    // Przekierowanie na stronę konta po zalogowaniu
    htmx.ajax("GET", "/htmx/moje-konto", {
      target: "#content",
      swap: "innerHTML",
    });
    htmx.history.push("/moje-konto");
  } else {
    // W przeciwnym razie wyświetlenie strony logowania
    htmx.ajax("GET", "/htmx/logowanie", {
      target: "#content",
      swap: "innerHTML",
    });
    htmx.history.push("/logowanie");
  }
});
