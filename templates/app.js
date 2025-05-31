// Plik app.js - UPROSZCZONA WERSJA STEROWANA ZDARZENIAMI Z SERWERA
console.log(
  "APP.JS (Simplified Server-Driven) LOADED. Timestamp:",
  new Date().toLocaleTimeString(),
);

// Listener htmx:configRequest (tylko dla globalnych nagłówków - BEZ ZMIAN)
document.body.addEventListener("htmx:configRequest", (event) => {
  if (!event.detail || !event.detail.headers) return;
  const guestCartId = localStorage.getItem("guestCartId");
  if (guestCartId) event.detail.headers["X-Guest-Cart-Id"] = guestCartId;
  const jwtToken = localStorage.getItem("jwtToken");
  if (jwtToken) event.detail.headers["Authorization"] = "Bearer " + jwtToken;
});

// Listener updateCartCount (dla koszyka - BEZ ZMIAN)
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
    window.scrollTo({ top: 0, behavior: "auto" });
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
  console.log(
    '<<<<< [App.js] "loginSuccessDetails" EVENT RECEIVED >>>>>. Detail:',
    JSON.stringify(evt.detail),
  );
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
  // Powiadomienie "showMessage" o sukcesie rejestracji powinno być już wywołane przez serwer.
  // Przekierowanie na stronę "Moje Konto" po udanej rejestracji i krótkim opóźnieniu
  setTimeout(() => {
    if (window.htmx) {
      // Najpierw wywołujemy zdarzenie, że użytkownik jest zalogowany (zakładając, że rejestracja = logowanie)
      // Jeśli serwer nie zwraca tokenu przy rejestracji, to ten krok trzeba pominąć lub obsłużyć inaczej.
      // Na razie zakładamy, że po rejestracji użytkownik nie jest automatycznie logowany,
      // więc tylko pokazujemy "dymek" i resetujemy formularz.
      // Jeśli chcesz automatyczne logowanie i przekierowanie, serwer przy rejestracji
      // musiałby również wysłać zdarzenie `loginSuccessDetails` z tokenem.
      // LUB przekierowujemy na stronę logowania:
      // window.htmx.ajax('GET', '/htmx/logowanie', {
      //   target: '#content', swap: 'innerHTML', pushUrl: '/logowanie'
      // });
      // LUB, jeśli rejestracja oznacza automatyczne zalogowanie i serwer wysłałby loginSuccessDetails:
      // (to by było obsługiwane przez listener loginSuccessDetails)

      // Na razie, po prostu pokazujemy sukces i resetujemy.
      // Jeśli chcesz przekierowanie na moje-konto, serwer musi wysłać loginSuccessDetails z tokenem
      // albo musisz tutaj wywołać logikę logowania.
      // Dla uproszczenia, na razie nie ma przekierowania po rejestracji.
      // Jeśli chcesz przekierowanie na moje-konto po rejestracji, musisz zmodyfikować backend,
      // aby po rejestracji zwracał również token i wywoływał `loginSuccessDetails`.
      // LUB, jeśli rejestracja nie loguje automatycznie, przekieruj na stronę logowania:
      console.log(
        "[App.js registrationComplete] Przekierowanie na stronę logowania po rejestracji.",
      );
      window.htmx.ajax("GET", "/htmx/logowanie", {
        target: "#content",
        swap: "innerHTML",
        pushUrl: "/logowanie",
      });
    }
  }, 1500);
});

// Usunęliśmy htmx:afterOnLoad. Komponent Alpine.js w index.html nasłuchuje na `window` dla `showMessage`.

console.log(
  "MEG JONI app.js (Simplified Server-Driven v2) loaded. Timestamp:",
  new Date().toLocaleTimeString(),
);

document.body.addEventListener("htmx:configRequest", function (evt) {
  console.log("HTMX Request path:", evt.detail.path);
  // Zmień 'guest_cart_id' na 'guestCartId'
  const guestCartId = localStorage.getItem("guestCartId");
  console.log("Guest cart ID from localStorage:", guestCartId);

  if (guestCartId && evt.detail.path.includes("/htmx/checkout")) {
    evt.detail.headers["x-guest-cart-id"] = guestCartId;
    console.log("Added header x-guest-cart-id:", guestCartId);
    console.log("All headers:", evt.detail.headers);
  }
});
