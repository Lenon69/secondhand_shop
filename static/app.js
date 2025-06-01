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

// --- Centralny listener authChangedClient ---
// Teraz głównie odpowiedzialny za pełne przeładowanie strony na "/"
document.addEventListener("authChangedClient", function (event) {
  console.log(
    "app.js: authChangedClient RECEIVED. isAuthenticated:",
    event.detail.isAuthenticated,
    "Source:",
    event.detail.source,
  );

  const isAuthenticated = event.detail.isAuthenticated;
  const source = event.detail.source;

  // Sprawdzamy, czy URL to już "/" aby uniknąć niepotrzebnego przeładowania,
  // chyba że jest to wymuszone (np. po jawnym logowaniu/wylogowaniu).
  const isAlreadyHome = window.location.pathname === "/";

  if (source === "login" && isAuthenticated) {
    // Komunikat o sukcesie logowania powinien być już wyświetlony przez HX-Trigger z serwera
    // lub przez listener 'loginSuccessDetails'.
    console.log(
      "app.js: authChangedClient - User logged in. Reloading to homepage.",
    );
    // Użyj replace, aby użytkownik nie mógł wrócić przyciskiem "wstecz" do strony logowania/konta
    if (!isAlreadyHome || event.detail.forceReload) window.location.href("/");
  } else if ((source === "logout" || source === "401") && !isAuthenticated) {
    // Komunikat o wylogowaniu lub wygaśnięciu sesji jest emitowany przez inne listenery.
    // Tutaj dodajemy opóźnienie, aby użytkownik zdążył zobaczyć komunikat przed przeładowaniem.
    console.log(
      "app.js: authChangedClient - User logged out or session expired. Reloading to homepage after delay.",
    );
    setTimeout(
      () => {
        if (!isAlreadyHome || event.detail.forceReload)
          window.location.href("/");
      },
      source === "401" ? 1 : 1,
    ); // Dłuższe opóźnienie dla komunikatu o błędzie 401
  }
  // Inne przypadki 'authChangedClient' (jeśli takie są i nie mają 'source') nie spowodują przeładowania.
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

// --- Listener dla "loginSuccessDetails" (z HX-Trigger od serwera) ---
document.body.addEventListener("loginSuccessDetails", function (evt) {
  console.log("loginSuccessDetails: Detail:", evt.detail);
  if (evt.detail && evt.detail.token) {
    localStorage.setItem("jwtToken", evt.detail.token);
    // Komunikat o sukcesie logowania jest już wysyłany przez serwer (HX-Trigger showMessage)
    // i powinien zostać wyświetlony przez komponent Toast w Alpine.js.
    // Czekamy chwilę, aby użytkownik mógł zobaczyć komunikat, a następnie przeładowujemy.
    console.log("Login successful. Reloading to homepage...");
    setTimeout(() => {
      window.location.replace("/"); // Pełne przeładowanie na stronę główną
    }, 700); // Krótkie opóźnienie na wyświetlenie komunikatu sukcesu
  } else {
    console.error(
      "[App.js] loginSuccessDetails event, but NO TOKEN:",
      evt.detail,
    );
    // Wyświetl błąd, jeśli token nie dotarł
    window.dispatchEvent(
      new CustomEvent("showMessage", {
        detail: {
          message: "Blad logowania: brak tokenu (klient).",
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
  }, 1);
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

// Listener htmx:responseError
document.body.addEventListener("htmx:responseError", function (evt) {
  const xhr = evt.detail.xhr;
  if (xhr.status === 401) {
    console.warn(
      "🔥 Otrzymano 401 Unauthorized – sesja mogła wygasnąć. Usuwam token.",
    );
    localStorage.removeItem("jwtToken");
    console.log("Token JWT usunięty z localStorage.");

    // Poinformuj Alpine.js o zmianie stanu (aby np. zaktualizował tekst linku)
    // To zdarzenie nie będzie już inicjować nawigacji HTMX, jeśli Alpine je tylko konsumuje do zmiany stanu.
    window.dispatchEvent(
      new CustomEvent("authChangedClient", {
        detail: {
          isAuthenticated: false,
          // Nie potrzebujemy już redirectUrl/pushUrl/source tutaj, jeśli zawsze jest pełny reload
        },
      }),
    );

    // Wyświetl komunikat dla użytkownika.
    window.dispatchEvent(
      new CustomEvent("showMessage", {
        detail: {
          message:
            "Twoja sesja wygasła lub nie masz uprawnień. Zaloguj się ponownie.",
          type: "warning",
        },
      }),
    );

    // Przeładuj stronę na stronę główną po chwili, aby użytkownik zobaczył komunikat.
    console.log("Session expired (401). Reloading to homepage after delay...");
    setTimeout(() => {
      window.location.replace("/"); // Pełne przeładowanie na stronę główną
    }, 1); // Opóźnienie na wyświetlenie komunikatu
  }
});
