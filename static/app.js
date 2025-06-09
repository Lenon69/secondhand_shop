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
  if (event.detail.requestConfig.headers["HX-History-Restore-Request"]) {
    return;
  }

  window.scrollTo({ top: 0, behavior: "auto" });

  // Pozostała logika czyszczenia komunikatów
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
  }
});

document.body.addEventListener(
  "htmx:afterSwap",
  document.body.addEventListener("htmx:afterSwap", function (event) {
    const target = event.detail.target;

    // Sprawdzamy, czy żądanie NIE było przywróceniem historii (ten kod już masz)
    if (!event.detail.requestConfig.headers["HX-History-Restore-Request"]) {
      if (target) {
        target.scrollIntoView({ behavior: "auto", block: "start" });
      }
    }

    // Obecna logika czyszczenia komunikatów - pozostaje bez zmian
    if (target.id === "content" || target.closest("#content")) {
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
    }
  }),
);

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

// --- Listener authChangedFromBackend (jeśli jest używany i ma powodować pełny reload) ---
document.body.addEventListener("authChangedFromBackend", function (evt) {
  if (evt.detail && typeof evt.detail.isAuthenticated !== "undefined") {
    let needsReload = false;
    if (evt.detail.token) {
      localStorage.setItem("jwtToken", evt.detail.token);
      if (evt.detail.isAuthenticated) needsReload = true; // np. po odświeżeniu tokenu
    } else if (!evt.detail.isAuthenticated) {
      localStorage.removeItem("jwtToken");
      needsReload = true; // np. po wylogowaniu przez serwer
    }

    // Poinformuj Alpine o zmianie stanu
    window.dispatchEvent(
      new CustomEvent("authChangedClient", {
        detail: { isAuthenticated: evt.detail.isAuthenticated },
      }),
    );

    if (needsReload) {
      console.log("authChangedFromBackend: Triggering homepage reload.");
      setTimeout(() => {
        // Daj czas na wyświetlenie ewentualnych komunikatów
        window.location.replace("/");
      }, 500);
    }
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
    console.log("Login successful. Reloading to homepage.");
    window.location.replace("/");
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

// --- Listener htmx:responseError ---
document.body.addEventListener("htmx:responseError", function (evt) {
  const xhr = evt.detail.xhr;
  const requestPath = evt.detail.requestConfig.path; // Ścieżka żądania, które zwróciło błąd

  if (xhr.status === 401) {
    if (requestPath === "/api/auth/login") {
      // Błąd 401 podczas próby logowania (np. złe hasło)
      // Serwer powinien wysłać HX-Trigger z komunikatem "Błędny email lub hasło"
      // Ten komunikat zostanie obsłużony przez Alpine Toast.
      // NIE przeładowujemy strony, użytkownik pozostaje na formularzu logowania.
      console.warn(
        "🔥 Błąd 401 (Nieautoryzowany) podczas próby logowania na:",
        requestPath,
      );
      // Nie usuwamy tokenu, bo użytkownik może go nie mieć lub próbuje się zalogować ponownie.
      // Nie emitujemy tutaj 'authChangedClient' ani nie robimy pełnego przeładowania.
      // Komunikat o błędzie logowania jest wysyłany z serwera przez HX-Trigger.
    } else {
      // Błąd 401 na innej ścieżce (prawdopodobnie wygasła sesja)
      console.warn(
        "🔥 Otrzymano 401 Unauthorized (prawdopodobnie wygasła sesja) dla ścieżki:",
        requestPath,
        ". Usuwam token.",
      );
      localStorage.removeItem("jwtToken");
      console.log("Token JWT usunięty z localStorage.");

      // Poinformuj Alpine.js o zmianie stanu (aby np. zaktualizował tekst linku)
      window.dispatchEvent(
        new CustomEvent("authChangedClient", {
          detail: { isAuthenticated: false },
        }),
      );

      // Wyświetl komunikat dla użytkownika.
      window.dispatchEvent(
        new CustomEvent("showMessage", {
          detail: {
            message:
              "Twoja sesja wygasla lub nie masz uprawnien. Zaloguj sie ponownie.",
            type: "warning",
          },
        }),
      );

      // Przeładuj stronę na stronę główną po chwili, aby użytkownik zobaczył komunikat.
      console.log(
        "Sesja wygasła (401) dla innej ścieżki. Przeładowuję stronę główną po opóźnieniu...",
      );
      setTimeout(() => {
        window.location.replace("/");
      }, 700);
    }
  }
});

document.body.addEventListener("orderPlaced", function (evt) {
  console.log("Order placed successfully:", evt.detail);
  // Przekieruj na stronę główną (lub inną stronę podsumowania)
  if (evt.detail.redirectTo) {
    // Daj czas na wyświetlenie komunikatu o sukcesie
    setTimeout(() => {
      window.location.replace(evt.detail.redirectTo);
    }, 1500); // 1.5 sekundy
  }
});

document.body.addEventListener("clearCartDisplay", function (evt) {
  console.log("Clearing cart display due to order placement.");
  // Wyemituj zdarzenie, które zaktualizuje licznik koszyka w Alpine.js na 0
  // i wyczyści wizualnie koszyk, jeśli jest otwarty.
  // To jest bardziej złożone, bo `updateCartCount` oczekuje pełnych danych koszyka.
  // Prostsze może być wywołanie przeładowania, które już się dzieje.
  // Alternatywnie, Alpine.js może nasłuchiwać na 'orderPlaced' i zresetować swój stan koszyka.
  // Na razie, pełne przeładowanie strony po 'orderPlaced' załatwi sprawę czyszczenia.
  // Można też wysłać specyficzne zdarzenie do Alpine:
  window.dispatchEvent(
    new CustomEvent("js-update-cart", {
      detail: { newCount: 0, newCartTotalPrice: 0 },
      bubbles: true,
    }),
  );
  // I zamknąć panel koszyka, jeśli jest otwarty (w Alpine)
  // window.dispatchEvent(new CustomEvent('closeCartPanel'));
});

function adminProductEditForm() {
  return {
    existingImagesOnInit: [],
    imagePreviews: Array(8).fill(null),
    imageFiles: Array(8).fill(null),
    imagesToDelete: [],
    productStatus: "",

    initAlpineComponent(initialImagesJson, currentStatusStr) {
      try {
        this.existingImagesOnInit = JSON.parse(initialImagesJson || "[]");
      } catch (e) {
        console.error(
          "Błąd parsowania initialImagesJson:",
          e,
          initialImagesJson,
        );
        this.existingImagesOnInit = [];
      }
      this.productStatus = currentStatusStr || "Available";

      this.imagePreviews = Array(8).fill(null);
      this.imageFiles = Array(8).fill(null);
      this.existingImagesOnInit.forEach((url, i) => {
        if (i < 8) {
          this.imagePreviews[i] = url;
        }
      });

      this.$watch("imagesToDelete", (newValue) => {
        const hiddenInput = document.getElementById(
          "urls_to_delete_hidden_input",
        );
        if (hiddenInput) {
          hiddenInput.value = JSON.stringify(newValue);
        }
      });

      const initialHiddenInput = document.getElementById(
        "urls_to_delete_hidden_input",
      );
      if (initialHiddenInput) {
        initialHiddenInput.value = JSON.stringify(this.imagesToDelete);
      }
    },

    getOriginalUrlForSlot(index) {
      // Zwraca oryginalny URL dla danego slotu, jeśli istniał przy inicjalizacji
      if (index < this.existingImagesOnInit.length) {
        return this.existingImagesOnInit[index];
      }
      return null;
    },

    handleFileChange(event, index) {
      const selectedFile = event.target.files[0];
      if (selectedFile) {
        const originalUrl = this.getOriginalUrlForSlot(index);
        if (originalUrl) {
          const deleteIdx = this.imagesToDelete.indexOf(originalUrl);
          if (deleteIdx > -1) {
            this.imagesToDelete.splice(deleteIdx, 1);
          }
        }
        this.imageFiles[index] = selectedFile;
        const reader = new FileReader();
        reader.onload = (e) => {
          this.$nextTick(() => {
            this.imagePreviews[index] = e.target.result;
          });
        };
        reader.readAsDataURL(selectedFile);
      } else {
        event.target.value = null;
      }
    },

    removeImage(index, inputId) {
      const originalUrl = this.getOriginalUrlForSlot(index);

      if (originalUrl && !this.imagesToDelete.includes(originalUrl)) {
        this.imagesToDelete.push(originalUrl);
      } else {
        this.imageFiles[index] = null;
        this.imagePreviews[index] = null;
        const fileInput = document.getElementById(inputId);
        if (fileInput) fileInput.value = null;
      }
    },

    cancelDeletion(index) {
      const originalUrl = this.getOriginalUrlForSlot(index);
      if (originalUrl) {
        const deleteIdx = this.imagesToDelete.indexOf(originalUrl);
        if (deleteIdx > -1) {
          this.imagesToDelete.splice(deleteIdx, 1);
        }
      }
    },

    isSlotFilled(index) {
      return this.imagePreviews[index] !== null;
    },

    getSlotImageSrc(index) {
      return this.imagePreviews[index];
    },

    isMarkedForDeletion(index) {
      const originalUrl = this.getOriginalUrlForSlot(index);
      return originalUrl && this.imagesToDelete.includes(originalUrl);
    },
  };
}

document.body.addEventListener("htmx:beforeSwap", function (event) {
  const xhr = event.detail.xhr;
  const requestConfig = event.detail.requestConfig;

  // Sprawdź, czy to odpowiedź z naszego formularza edycji produktu
  // (metoda PATCH na ścieżkę /api/products/{uuid})
  const productApiPatchRegex =
    /^\/api\/products\/[0-9a-fA-F]{8}-([0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}$/;

  if (
    requestConfig &&
    requestConfig.verb &&
    requestConfig.verb.toLowerCase() === "patch" &&
    requestConfig.path &&
    productApiPatchRegex.test(requestConfig.path)
  ) {
    if (xhr && xhr.status === 200) {
      try {
        const responseJson = JSON.parse(xhr.responseText);
        // Proste sprawdzenie, czy odpowiedź wygląda jak obiekt produktu (posiada np. 'id' i 'name')
        // Możesz to dostosować, jeśli potrzebujesz bardziej szczegółowej weryfikacji.
        if (
          responseJson &&
          typeof responseJson.id !== "undefined" &&
          typeof responseJson.name !== "undefined"
        ) {
          console.log(
            "Pomyślna aktualizacja produktu, odpowiedź JSON przechwycona.",
          );

          // 1. Wywołaj zdarzenie, aby wyświetlić Twój toast/bąbelek
          window.dispatchEvent(
            new CustomEvent("showMessage", {
              detail: {
                message: "Pomyślnie zapisano zmiany",
                type: "success", // lub inny typ, którego używa Twój system toastów
              },
            }),
          );

          // 2. Anuluj standardową operację podmiany treści przez HTMX
          //    (aby nie wstawiać JSONa do `#edit-product-messages`)
          event.detail.shouldSwap = false;

          // 3. Opcjonalnie: Wyczyść div #edit-product-messages lub wstaw tam statyczny komunikat,
          //    jeśli chcesz, aby coś tam się pojawiło zamiast JSONa.
          //    Jeśli toast jest wystarczający, możesz zostawić to pole puste.
          const targetElement = event.detail.target; // To powinien być #edit-product-messages
          if (targetElement) {
            targetElement.innerHTML = ""; // Czyści zawartość
          }

          // 4.
          if (window.htmx) {
            htmx.ajax("GET", "/htmx/admin/products", {
              target: "#admin-content",
              swap: "innerHTML",
              pushUrl: true,
            });
          }
        }
        return;
        // Jeśli JSON nie jest oczekiwanym obiektem produktu, pozwól HTMX działać domyślnie
        // (może to być np. odpowiedź błędu walidacji w formacie HTML/JSON od serwera)
      } catch (e) {
        // Jeśli odpowiedź nie jest JSONem, pozwól HTMX działać domyślnie
        console.warn(
          "Odpowiedź z aktualizacji produktu nie była oczekiwanym JSONem lub wystąpił błąd parsowania:",
          e,
        );
      }
    }
    // Jeśli status nie jest 200 (np. błąd walidacji 422), pozwól HTMX działać domyślnie,
    // aby wyświetlić ewentualne komunikaty błędów w #edit-product-messages.
  }
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
  } catch (_) {}
});

document.addEventListener("DOMContentLoaded", function () {
  const globalSpinner = document.getElementById("global-loading-spinner");

  if (globalSpinner) {
    document.body.addEventListener("htmx:beforeRequest", function (event) {
      if (event.detail.requestConfig.headers["HX-History-Restore-Request"]) {
        return; // Zakończ i nie pokazuj spinnera
      }
      globalSpinner.classList.add("show");
    });

    document.body.addEventListener("htmx:afterRequest", function () {
      globalSpinner.classList.remove("show");
    });

    // Zostawiamy na wszelki wypadek, gdyby wystąpił błąd sieciowy
    document.body.addEventListener("htmx:sendError", function () {
      globalSpinner.classList.remove("show");
    });
    document.body.addEventListener("htmx:responseError", function () {
      globalSpinner.classList.remove("show");
    });
  } else {
    console.error("Global spinner element #global-loading-spinner NOT FOUND!");
  }
});
