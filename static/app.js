/**
 * Wykrywa skalowanie systemowe i dodaje klasę do body,
 * aby umożliwić specyficzne poprawki w CSS.
 */
function handleScreenScaling() {
  // window.devicePixelRatio informuje nas o poziomie skalowania.
  // 1.0 = 100%, 1.25 = 125%, 1.5 = 150% itd.
  const dpr = window.devicePixelRatio || 1;
  const body = document.body;

  // Usuń poprzednie klasy, aby uniknąć konfliktów
  body.classList.remove("dpr-100", "dpr-125", "dpr-150", "dpr-gt-150");

  if (dpr <= 1.1) {
    body.classList.add("dpr-100"); // Standardowe skalowanie
  } else if (dpr > 1.1 && dpr <= 1.35) {
    body.classList.add("dpr-125"); // Skalowanie 125%
  } else if (dpr > 1.35 && dpr <= 1.6) {
    body.classList.add("dpr-150"); // Skalowanie 150%
  } else {
    body.classList.add("dpr-gt-150"); // Skalowanie powyżej 150%
  }

  console.log(
    `[Skalowanie] Wykryto devicePixelRatio: ${dpr}. Dodano klasę: ${body.classList[body.classList.length - 1]}`,
  );
}

// Wywołaj funkcję przy ładowaniu strony
document.addEventListener("DOMContentLoaded", handleScreenScaling);

// Wywołaj ponownie przy zmianie rozmiaru okna (użyteczne przy przenoszeniu okna między monitorami)
window.addEventListener("resize", handleScreenScaling);

/**
 * Główny plik JavaScript aplikacji MEG JONI
 * Wersja zrefaktoryzowana, zoptymalizowana i poprawiona.
 *

// ========================================================================
// I. GŁÓWNA INICJALIZACJA I LISTENERY
// ========================================================================
*/
// Wszystkie listenery inicjujemy po załadowaniu struktury strony (DOM).
document.addEventListener("DOMContentLoaded", function () {
  checkSession();
  restoreScrollPosition();

  // Delegacja zdarzeń do zapisywania pozycji przy kliknięciu linku produktu.
  document.body.addEventListener("click", function (event) {
    const productLink = event.target.closest('a[href^="/produkty/"]');
    if (productLink) {
      saveScrollPositionForProductLink();
    }
  });

  const globalSpinner = document.getElementById("global-loading-spinner");
  if (!globalSpinner) {
    console.error("Global spinner element #global-loading-spinner NOT FOUND!");
    return;
  }

  const hideSpinner = () => {
    globalSpinner.classList.remove("show");
  };

  // Pokaż spinner przed każdym żądaniem HTMX
  document.body.addEventListener("htmx:beforeRequest", (event) => {
    globalSpinner.classList.add("show");
    // Sprawdzamy, czy to żądanie do strony głównej, aby wymusić pełny reload
    const path = event.detail.requestConfig.path;
    if (path === "/" || path === "") {
      event.preventDefault();
      window.location.href = "/";
    }
    return;
  });

  // Schowaj spinner po każdym zakończonym żądaniu (sukces lub błąd)
  document.body.addEventListener("htmx:afterRequest", function (event) {
    // Sprawdzamy, czy żądanie zakończyło się sukcesem (status 2xx)
    if (!event.detail.successful) {
      hideSpinner();
      return;
    }

    const requestConfig = event.detail.requestConfig;
    const requestPath = requestConfig.path;

    const scrollDataJSON = localStorage.getItem(SCROLL_RESTORATION_KEY);
    let isRestorePending = false;
    if (scrollDataJSON) {
      try {
        // Sprawdzamy, czy URL powrotny w localStorage zgadza się z URL, na który właśnie weszliśmy
        const scrollData = JSON.parse(scrollDataJSON);
        if (scrollData.returnUrl === window.location.href) {
          isRestorePending = true;
        }
      } catch (e) {}
    }

    // Definiujemy listę ścieżek, DLA KTÓRYCH NIE CHCEMY przewijać do góry.
    const noScrollPaths = [
      "/htmx/cart/remove/",
      "/htmx/cart/toggle/",
      "/htmx/cart/details",
      "/api/orders/", // Aktualizacja statusu zamówienia w tle
      "/api/products/", // Archiwizacja produktu w tle
    ];

    // Sprawdzamy, czy aktualna ścieżka żądania NIE ZACZYNA SIĘ od żadnej z powyższych.
    const shouldScrollToTop = !noScrollPaths.some((path) =>
      requestPath.startsWith(path),
    );
    const isHistoryRestore =
      requestConfig.headers["HX-History-Restore-Request"];

    if (shouldScrollToTop && !isHistoryRestore && !isRestorePending) {
      console.log(
        `[Scroll] Wymuszam przewinięcie do góry dla ścieżki: ${requestPath}`,
      );
      window.scrollTo({ top: 0, left: 0, behavior: "auto" });
    }

    // Po podmianie treści przez HTMX, próbujemy przywrócić pozycję przewijania
    document.body.addEventListener("htmx:afterSwap", () => {
      restoreScrollPosition();
    });

    hideSpinner();
  });
  document.body.addEventListener("htmx:sendError", hideSpinner);
  document.body.addEventListener("htmx:responseError", hideSpinner);
  document.body.addEventListener("logoutClient", () => {
    clientSideLogout();
  });

  (function () {
    let isReloading = false;

    const forceReload = (sourceEvent) => {
      if (isReloading) {
        return;
      }
      isReloading = true;
      console.log(
        `Wykryto nawigację "Wstecz" przez "${sourceEvent}". Wymuszam przeładowanie...`,
      );
      window.location.reload();
    };

    window.addEventListener("pageshow", function (event) {
      if (event.persisted) {
        forceReload("pageshow - event.persisted");
      }
    });

    document.body.addEventListener("htmx:historyRestore", function () {
      forceReload("htmx:historyRestore");
    });
  })();
});

initEventListeners();
function initEventListeners() {
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

  document.body.addEventListener("htmx:afterSwap", (event) => {
    // Czyszczenie starych komunikatów z formularzy logowania/rejestracji
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
      fetch("/api/auth/logout", { method: "POST" }).catch((err) =>
        console.error("Błąd podczas serwerowego wylogowania:", err),
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
              "Twoja sesja wygasla lub nie masz uprawnien. Zaloguj sie ponownie.",
            type: "warning",
            duration: 3000,
          },
        }),
      );

      // Przeładuj na stronę główną po chwili
      setTimeout(() => window.location.replace("/"), 0);
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
              detail: {
                message: "Pomyślnie zapisano zmiany",
                type: "success",
              },
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
    }, 0); // Małe opóźnienie dla pewności
  });

  /**
   * Czyści wizualnie stan koszyka (używane po złożeniu zamówienia).
   */
  // NOWA, POPRAWIONA WERSJA w app.js
  document.body.addEventListener("clearCartDisplay", () => {
    console.log(
      "Czyszczenie i automatyczne odświeżanie koszyka po zamówieniu.",
    );

    // Krok 1: Zaktualizuj stan licznika w Alpine.js (bez zmian)
    window.dispatchEvent(
      new CustomEvent("js-update-cart", {
        detail: { newCount: 0, newCartTotalPrice: 0 },
        bubbles: true,
      }),
    );

    // KROK 2: BEZPOŚREDNIE ŻĄDANIE ODŚWIEŻENIA KOSZYKA
    // Używamy htmx.ajax, aby natychmiast pobrać nowy (pusty) widok koszyka
    // z serwera i wstawić go do panelu bocznego w tle.
    htmx.ajax("GET", "/htmx/cart/details", {
      target: "#cart-content-target",
      swap: "innerHTML",
    });
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
    imagePreviews: Array(10).fill(null),
    imageFiles: Array(10).fill(null),
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
        if (i < 10) this.imagePreviews[i] = url;
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
      return !!this.imagePreviews[index];
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

/**
 * Parsuje token JWT, aby uzyskać dostęp do jego zawartości (payload).
 * @param {string} token - Token JWT.
 * @returns {object|null} - Zdekodowany payload lub null w przypadku błędu.
 */
function parseJwt(token) {
  try {
    const base64Url = token.split(".")[1];
    const base64 = base64Url.replace(/-/g, "+").replace(/_/g, "/");
    const jsonPayload = decodeURIComponent(
      atob(base64)
        .split("")
        .map(function (c) {
          return "%" + ("00" + c.charCodeAt(0).toString(16)).slice(-2);
        })
        .join(""),
    );
    return JSON.parse(jsonPayload);
  } catch (e) {
    return null;
  }
}

/**
 * Wylogowuje użytkownika po stronie klienta.
 * Czyści localStorage, informuje użytkownika i odświeża stronę.
 */
function clientSideLogout() {
  console.log(
    "Wykonywanie wylogowania po stronie klienta (czyszczenie localStorage i przekierowanie)...",
  );
  localStorage.removeItem("jwtToken");

  window.dispatchEvent(
    new CustomEvent("showMessage", {
      detail: { type: "info", message: "Zostałes pomyslnie wylogowany." },
    }),
  );

  setTimeout(() => {
    window.location.href = "/";
    const globalSpinner = document.getElementById("global-loading-spinner");
    if (!globalSpinner) {
      console.error(
        "Global spinner element #global-loading-spinner NOT FOUND!",
      );
      return;
    }
    globalSpinner.classList.add("show");
  }, 0);
}

/**
 * Sprawdza ważność sesji użytkownika przy każdym załadowaniu strony.
 * Jeśli token wygasł, automatycznie wylogowuje.
 */
function checkSession() {
  const token = localStorage.getItem("jwtToken");
  if (!token) {
    return; // Brak tokenu, użytkownik jest gościem.
  }

  const decodedToken = parseJwt(token);
  if (!decodedToken || !decodedToken.exp) {
    // Token jest uszkodzony, wyloguj dla bezpieczeństwa
    clientSideLogout();
    return;
  }

  // `decodedToken.exp` jest w sekundach, a `Date.now()` w milisekundach.
  const isExpired = decodedToken.exp < Date.now() / 1000;

  if (isExpired) {
    clientSideLogout();
  } else {
    console.log(
      "Sesja jest ważna. Wygasa za:",
      new Date(decodedToken.exp * 1000),
    );
  }
}

/**
 * ========================================================================
 * IV. ZARZĄDZANIE POZYCJĄ PRZEWIJANIA
 * ========================================================================
 * Mechanizm zapisywania i przywracania pozycji przewijania, aby nawigacja
 * "Wstecz" była bardziej naturalna, nawet przy pełnym przeładowaniu.
 */

const SCROLL_RESTORATION_KEY = "productScrollPosition";

/**
 * Zapisuje pozycję przewijania i URL powrotny w localStorage.
 * Wywoływana przy kliknięciu linku prowadzącego do produktu.
 */
function saveScrollPositionForProductLink() {
  try {
    const scrollData = {
      position: window.scrollY,
      returnUrl: window.location.href,
    };
    localStorage.setItem(SCROLL_RESTORATION_KEY, JSON.stringify(scrollData));
    console.log(
      `[Scroll] Zapisano pozycję do przywrócenia: ${scrollData.position} dla URL: ${scrollData.returnUrl}`,
    );
  } catch (e) {
    console.error("[Scroll] Błąd zapisu pozycji do localStorage:", e);
  }
}

/**
 * Sprawdza, czy dla bieżącego URL istnieje zapisana pozycja przewijania.
 * Jeśli tak, przywraca ją i usuwa dane z localStorage.
 * @returns {boolean} - Zwraca true, jeśli pozycja została przywrócona, w przeciwnym razie false.
 */
function restoreScrollPosition() {
  try {
    const scrollDataJSON = localStorage.getItem(SCROLL_RESTORATION_KEY);
    if (!scrollDataJSON) return false;

    const scrollData = JSON.parse(scrollDataJSON);

    // KLUCZOWY WARUNEK: przywracaj tylko, jeśli URL się zgadza.
    if (scrollData.returnUrl === window.location.href) {
      console.log(`[Scroll] Przywracam pozycję: ${scrollData.position}`);
      setTimeout(() => {
        window.scrollTo({ top: scrollData.position, behavior: "auto" });
        // Usuwamy klucz po udanym przywróceniu.
        localStorage.removeItem(SCROLL_RESTORATION_KEY);
      }, 100); // Niewielkie opóźnienie, by DOM zdążył się wyrenderować.
      return true; // Sygnalizujemy, że przywróciliśmy pozycję.
    } else {
      // Jeśli URL się nie zgadza, czyścimy nieaktualny wpis.
      localStorage.removeItem(SCROLL_RESTORATION_KEY);
    }
  } catch (e) {
    console.error("[Scroll] Błąd przywracania pozycji:", e);
    localStorage.removeItem(SCROLL_RESTORATION_KEY);
  }
  return false;
}
