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
    existingImagesOnInit: [], // Tablica URLi istniejących obrazków (stringi)
    imagePreviews: Array(8).fill(null), // Podglądy (URL dla istniejących, base64 dla nowych)
    imageFiles: Array(8).fill(null), // Obiekty File dla nowo dodanych obrazków
    imagesToDelete: [], // Tablica URLi istniejących obrazków do usunięcia
    productStatus: "", // Aktualny status produktu

    initAlpineComponent(initialImagesJson, currentStatusStr) {
      console.log("Inicjalizacja adminProductEditForm...");
      console.log("Odebrane initialImagesJson:", initialImagesJson);
      console.log("Odebrany currentStatusStr:", currentStatusStr);
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

      this.productStatus = currentStatusStr || "Available"; // Ustaw domyślny, jeśli brak

      // Inicjalizuj imagePreviews
      this.imagePreviews = [...this.existingImagesOnInit];
      while (this.imagePreviews.length < 8) {
        this.imagePreviews.push(null);
      }
      console.log("Zainicjowane imagePreviews:", this.imagePreviews);
      console.log("Zainicjowany productStatus:", this.productStatus);
    },

    getOriginalUrlForSlot(index) {
      return this.existingImagesOnInit[index] || null;
    },

    handleFileChange(event, index) {
      const file = event.target.files[0];
      if (file) {
        this.imageFiles[index] = file; // Zapisz obiekt File
        const reader = new FileReader();
        reader.onload = (e) => {
          // Aktualizuj podgląd dla tego slotu
          // Użyj $nextTick, aby Alpine zdążył zaktualizować DOM, jeśli jest to potrzebne,
          // chociaż bezpośrednie przypisanie powinno działać dla reaktywności.
          this.$nextTick(() => {
            this.imagePreviews[index] = e.target.result;
          });
        };
        reader.readAsDataURL(file);

        // Jeśli ten slot miał wcześniej istniejący obrazek i jest on na liście do usunięcia,
        // usuń go z tej listy, bo użytkownik go nadpisuje.
        const originalUrl = this.getOriginalUrlForSlot(index);
        if (originalUrl) {
          const deleteIdx = this.imagesToDelete.indexOf(originalUrl);
          if (deleteIdx > -1) {
            this.imagesToDelete.splice(deleteIdx, 1);
          }
        }
      } else {
        // Użytkownik anulował wybór pliku
        const originalUrl = this.getOriginalUrlForSlot(index);
        if (originalUrl && this.imagePreviews[index] !== originalUrl) {
          // Jeśli był tam oryginalny obrazek i podgląd został zmieniony (np. na base64), przywróć go.
          this.imagePreviews[index] = originalUrl;
          this.imageFiles[index] = null; // Upewnij się, że nie ma tu pliku
        } else if (!originalUrl) {
          // Jeśli to był pusty slot na nowy obrazek
          this.imagePreviews[index] = null;
          this.imageFiles[index] = null;
        }
        // event.target.value = null; // To może być potrzebne, aby umożliwić ponowny wybór tego samego pliku, ale testuj
      }
    },

    removeImage(index, inputId) {
      const originalUrl = this.getOriginalUrlForSlot(index);

      // Jeśli usuwamy podgląd, który był oryginalnym, istniejącym obrazkiem,
      // i nie jest jeszcze na liście do usunięcia, dodaj go.
      if (
        originalUrl &&
        this.imagePreviews[index] === originalUrl &&
        !this.imagesToDelete.includes(originalUrl)
      ) {
        this.imagesToDelete.push(originalUrl);
        console.log(
          "Dodano do usunięcia (imagesToDelete):",
          originalUrl,
          this.imagesToDelete,
        );
      }

      // Zawsze czyść podgląd i plik dla tego slotu
      this.imagePreviews[index] = null;
      this.imageFiles[index] = null;

      const fileInput = document.getElementById(inputId);
      if (fileInput) {
        fileInput.value = null; // Wyczyść wartość inputu <input type="file">
      }
    },

    cancelDeletion(index) {
      const originalUrl = this.getOriginalUrlForSlot(index);
      if (originalUrl) {
        const deleteIdx = this.imagesToDelete.indexOf(originalUrl);
        if (deleteIdx > -1) {
          this.imagesToDelete.splice(deleteIdx, 1);
          console.log(
            "Anulowano usunięcie dla:",
            originalUrl,
            this.imagesToDelete,
          );
          // Przywróć oryginalny podgląd, tylko jeśli slot nie jest teraz zajęty przez nowo wybrany plik
          if (this.imageFiles[index] === null) {
            this.imagePreviews[index] = originalUrl;
          }
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
      // Jest oznaczony do usunięcia, jeśli jest na liście imagesToDelete
      // ORAZ nie ma w tym slocie nowo dodanego pliku (imageFiles[index] jest null)
      // ORAZ podgląd (imagePreviews[index]) został wyczyszczony (co robi removeImage)
      // LUB podgląd jest oryginalnym URLem, ale jest na liście do usunięcia.
      // Prostsza logika: jeśli jest na liście `imagesToDelete` i nie ma nowego pliku nadpisującego.
      return (
        originalUrl &&
        this.imagesToDelete.includes(originalUrl) &&
        this.imageFiles[index] === null
      );
    },
  };
}
