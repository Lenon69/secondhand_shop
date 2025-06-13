/**
 * Główny plik JavaScript aplikacji MEG JONI
 * Wersja zrefaktoryzowana, zoptymalizowana i poprawiona.
 *

// ========================================================================
// I. GŁÓWNA INICJALIZACJA I LISTENERY
// ========================================================================
*/

// Wszystkie listenery inicjujemy po załadowaniu struktury strony (DOM).
document.addEventListener(
  "DOMContentLoaded",
  document.addEventListener("DOMContentLoaded", function () {
    const globalSpinner = document.getElementById("global-loading-spinner");

    if (!globalSpinner) {
      console.error(
        "Global spinner element #global-loading-spinner NOT FOUND!",
      );
      return;
    }

    const hideSpinner = () => {
      globalSpinner.classList.remove("show");
    };

    // 1. ZAWSZE pokazuj spinner przed wysłaniem żądania HTMX.
    document.body.addEventListener("htmx:beforeRequest", function (event) {
      // Sprawdzamy, czy to żądanie do strony głównej, aby wymusić pełny reload
      // i uniknąć zablokowania spinnera (z poprzedniej poprawki).
      const path = event.detail.requestConfig.path;
      if (path === "/" || path === "") {
        event.preventDefault();
        window.location.href = "/";
        return;
      }
      globalSpinner.classList.add("show");
    });

    // 2. ZAWSZE chowaj spinner po zakończeniu ZWYKŁEGO żądania HTMX.
    document.body.addEventListener("htmx:afterRequest", hideSpinner);

    // 3. ZAWSZE chowaj spinner w razie jakiegokolwiek błędu.
    document.body.addEventListener("htmx:sendError", hideSpinner);
    document.body.addEventListener("htmx:responseError", hideSpinner);

    // 4. (NAJWAŻNIEJSZE) Specjalna obsługa przycisku "Wstecz"/"Dalej".
    // Używamy natywnego zdarzenia przeglądarki 'pageshow'.
    window.addEventListener("pageshow", function (event) {
      // event.persisted jest 'true', gdy strona jest przywracana z BFCache
      // (co dzieje się po kliknięciu "Wstecz").
      if (event.persisted) {
        // Dajemy przeglądarce 200ms na odmalowanie widoku, a potem
        // chowamy spinner, który mógł zostać "zamrożony" w stanie widocznym.
        setTimeout(hideSpinner, 200);
      }
    });
  }),

  initEventListeners(),
);

function initEventListeners() {
  // ========================================================================
  // A. Konfiguracja i cykl życia HTMX
  // ========================================================================

  /**
   * Dodaje nagłówki autoryzacji (JWT) i koszyka gościa do każdego żądania HTMX.
   */
  document.body.addEventListener("htmx:configRequest", (event) => {
    if (!event.detail?.headers) return;

    const guestCartId = localStorage.getItem("guestCartId");
    if (guestCartId) {
      event.detail.headers["X-Guest-Cart-Id"] = guestCartId;
    }

    const jwtToken = localStorage.getItem("jwtToken");
    if (jwtToken) {
      event.detail.headers["Authorization"] = `Bearer ${jwtToken}`;
    }
  });

  /**
   * Główna logika po podmianie treści przez HTMX.
   * Odpowiada za przewijanie strony do góry i czyszczenie komunikatów.
   */
  document.body.addEventListener("htmx:afterSwap", (event) => {
    // 1. Niezawodne przewijanie do góry (top: 0)
    if (!event.detail.requestConfig.headers["HX-History-Restore-Request"]) {
      // setTimeout z opóźnieniem 0 daje przeglądarce czas na dokończenie
      // renderowania, co gwarantuje, że przewinięcie zadziała poprawnie.
      setTimeout(() => {
        window.scrollTo({ top: 0, left: 0, behavior: "auto" });
      }, 0);
    }

    // 2. Czyszczenie starych komunikatów z formularzy logowania/rejestracji
    const isContentSwap =
      event.detail.target.id === "content" ||
      event.detail.target.closest("#content");
    const isAuthPage =
      window.location.pathname.endsWith("/logowanie") ||
      window.location.pathname.endsWith("/rejestracja");

    if (isContentSwap && !isAuthPage) {
      const loginMessages = document.getElementById("login-messages");
      if (loginMessages) loginMessages.innerHTML = "";

      const registrationMessages = document.getElementById(
        "registration-messages",
      );
      if (registrationMessages) registrationMessages.innerHTML = "";
    }
  });

  /**
   * Przechwytuje błędy odpowiedzi z serwera, głównie dla obsługi sesji (401).
   */
  document.body.addEventListener("htmx:responseError", (event) => {
    const xhr = event.detail.xhr;
    const requestPath = event.detail.requestConfig.path;

    if (xhr.status === 401 && requestPath !== "/api/auth/login") {
      // Błąd 401 na dowolnej ścieżce innej niż logowanie = wygasła sesja
      console.warn(
        `Wygasła sesja (401) dla ścieżki: ${requestPath}. Usuwam token i przeładowuję stronę.`,
      );
      localStorage.removeItem("jwtToken");

      // Poinformuj Alpine.js o zmianie stanu (np. żeby zaktualizował UI)
      window.dispatchEvent(
        new CustomEvent("authChangedClient", {
          detail: { isAuthenticated: false, source: "401" },
        }),
      );

      // Wyświetl komunikat dla użytkownika
      window.dispatchEvent(
        new CustomEvent("showMessage", {
          detail: {
            message:
              "Twoja sesja wygasła lub nie masz uprawnień. Zaloguj się ponownie.",
            type: "warning",
          },
        }),
      );

      // Przeładuj na stronę główną po chwili
      setTimeout(() => window.location.replace("/"), 800);
    }
  });

  /**
   * Przechwytuje odpowiedź z udanej aktualizacji produktu (PATCH)
   * aby wyświetlić komunikat i przeładować listę, zamiast wstawiać JSON na stronę.
   */
  document.body.addEventListener("htmx:beforeSwap", (event) => {
    const { xhr, requestConfig, target } = event.detail;
    const isProductPatch =
      requestConfig.verb?.toLowerCase() === "patch" &&
      /^\/api\/products\//.test(requestConfig.path);

    if (isProductPatch && xhr?.status === 200) {
      try {
        const responseJson = JSON.parse(xhr.responseText);
        if (responseJson?.id && responseJson?.name) {
          console.log(
            "Pomyślna aktualizacja produktu, przechwycono odpowiedź.",
          );

          // Anuluj domyślną podmianę (aby nie wstawiać JSON-a do HTML)
          event.detail.shouldSwap = false;
          if (target) target.innerHTML = ""; // Wyczyść kontener na komunikaty

          // Pokaż toast o sukcesie
          window.dispatchEvent(
            new CustomEvent("showMessage", {
              detail: { message: "Pomyślnie zapisano zmiany", type: "success" },
            }),
          );

          // Przeładuj widok listy produktów w panelu admina
          htmx.ajax("GET", "/htmx/admin/products", {
            target: "#admin-content",
            swap: "innerHTML",
            pushUrl: true,
          });
        }
      } catch (e) {
        console.warn(
          "Odpowiedź z aktualizacji produktu nie była oczekiwanym JSONem.",
          e,
        );
      }
    }
  });

  /**
   * Ogólny listener, który szuka w odpowiedziach JSON klucza 'showMessage'
   * i wyzwala toast, jeśli go znajdzie.
   */
  document.body.addEventListener("htmx:afterOnLoad", (event) => {
    try {
      const json = JSON.parse(event.detail.xhr.responseText);
      if (json.showMessage) {
        window.dispatchEvent(
          new CustomEvent("showMessage", { detail: json.showMessage }),
        );
      }
    } catch (_) {
      // Ignoruj błędy parsowania, odpowiedź mogła nie być JSON-em.
    }
  });

  // ========================================================================
  // B. Obsługa niestandardowych zdarzeń aplikacji (z HX-Trigger)
  // ========================================================================

  /**
   * Aktualizuje licznik koszyka i sumę częściową na podstawie danych z serwera.
   */
  document.body.addEventListener("updateCartCount", (event) => {
    if (!event.detail) return;

    // Przekaż zdarzenie dalej do Alpine.js
    document.body.dispatchEvent(
      new CustomEvent("js-update-cart", {
        detail: event.detail,
        bubbles: true,
      }),
    );

    // Zaktualizuj sumę w panelu bocznym koszyka
    if (typeof event.detail.newCartTotalPrice !== "undefined") {
      const el = document.getElementById("cart-subtotal-price");
      if (el) {
        const price = (
          parseInt(event.detail.newCartTotalPrice, 10) / 100
        ).toFixed(2);
        el.innerHTML = `${price.replace(".", ",")} zł`;
      }
    }
  });

  /**
   * Obsługuje pomyślne zalogowanie. Zapisuje token i przeładowuje stronę.
   */
  document.body.addEventListener("loginSuccessDetails", (event) => {
    if (event.detail?.token) {
      localStorage.setItem("jwtToken", event.detail.token);
      console.log("Logowanie pomyślne. Przeładowuję stronę...");
      window.location.replace("/"); // Użyj replace, by użytkownik nie wrócił do strony logowania
    } else {
      console.error(
        "Otrzymano zdarzenie loginSuccessDetails, ale bez tokenu JWT!",
        event.detail,
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

  /**
   * Obsługuje pomyślną rejestrację. Resetuje formularz i przekierowuje na logowanie.
   */
  document.body.addEventListener("registrationComplete", () => {
    console.log("Rejestracja zakończona. Przekierowuję na logowanie.");
    const form = document.getElementById("registration-form");
    if (form) form.reset();

    // Daj interfejsowi chwilę na odświeżenie i przekieruj na logowanie
    setTimeout(() => {
      htmx.ajax("GET", "/htmx/logowanie", {
        target: "#content",
        swap: "innerHTML",
        pushUrl: "/logowanie",
      });
    }, 100); // Małe opóźnienie dla pewności
  });

  /**
   * Obsługuje pomyślne złożenie zamówienia. Przekierowuje na stronę główną.
   */
  document.body.addEventListener("orderPlaced", (event) => {
    console.log("Zamówienie złożone pomyślnie:", event.detail);
    if (event.detail.redirectTo) {
      setTimeout(() => {
        window.location.replace(event.detail.redirectTo);
      }, 1500); // Opóźnienie, aby użytkownik zdążył zobaczyć komunikat o sukcesie
    }
  });

  /**
   * Czyści wizualnie stan koszyka (używane po złożeniu zamówienia).
   */
  document.body.addEventListener("clearCartDisplay", () => {
    console.log("Czyszczenie wyświetlania koszyka po zamówieniu.");
    window.dispatchEvent(
      new CustomEvent("js-update-cart", {
        detail: { newCount: 0, newCartTotalPrice: 0 },
        bubbles: true,
      }),
    );
  });

  /**
   * Nasłuchuje na zdarzenie zmiany stanu autoryzacji, ale GŁÓWNIE
   * do informowania innych części aplikacji (jak Alpine.js).
   * Logika przeładowania strony została przeniesiona do bardziej
   * specyficznych handlerów (loginSuccessDetails, htmx:responseError).
   */
  document.addEventListener("authChangedClient", (event) => {
    console.log(
      `Zdarzenie authChangedClient, źródło: ${event.detail?.source || "nieznane"}`,
    );
    // Ten listener głównie informuje Alpine.js.
    // Specyficzne akcje (jak przeładowanie) są obsługiwane przez zdarzenia, które go wywołały.
  });
}

// ========================================================================
// III. KOMPONENTY ALPINE.JS (udostępnione globalnie)
// ========================================================================

/**
 * Zwraca obiekt dla komponentu Alpine.js do zarządzania
 * formularzem edycji/dodawania produktu w panelu admina.
 */
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

      this.imagePreviews.fill(null);
      this.imageFiles.fill(null);
      this.existingImagesOnInit.forEach((url, i) => {
        if (i < 8) this.imagePreviews[i] = url;
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
      return this.existingImagesOnInit[index] || null;
    },

    handleFileChange(event, index) {
      const selectedFile = event.target.files[0];
      if (!selectedFile) {
        event.target.value = null;
        return;
      }

      const originalUrl = this.getOriginalUrlForSlot(index);
      if (originalUrl) {
        // Jeśli podmieniamy istniejący obraz, upewniamy się, że nie jest on oznaczony do usunięcia
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
    },

    removeImage(index, inputId) {
      const originalUrl = this.getOriginalUrlForSlot(index);

      if (originalUrl && !this.imagesToDelete.includes(originalUrl)) {
        // Jeśli to jest istniejący obraz z serwera, oznacz go do usunięcia
        this.imagesToDelete.push(originalUrl);
      } else {
        // Jeśli to jest nowo dodany podgląd, po prostu go usuń
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
