// ZAKTUALIZOWANY Listener htmx:afterOnLoad
document.body.addEventListener("htmx:afterOnLoad", function (evt) {
  if (!evt.detail || !evt.detail.xhr) {
    console.warn("[App.js:afterOnLoad] Event detail or XHR missing.");
    return;
  }

  const xhr = evt.detail.xhr;
  const triggeringElement = evt.detail.elt; // Element, który wywołał żądanie (np. formularz)
  const targetElement = evt.detail.target; // Element, który miałby otrzymać odpowiedź (hx-target)

  console.log(
    "[App.js:afterOnLoad] Triggering Element ID:",
    triggeringElement ? triggeringElement.id : "N/A",
  );
  console.log(
    "[App.js:afterOnLoad] Target Element ID:",
    targetElement ? targetElement.id : "N/A",
  );
  console.log(
    "[App.js:afterOnLoad] Request successful:",
    evt.detail.successful,
  );
  console.log("[App.js:afterOnLoad] XHR Status:", xhr.status);

  const dispatchNotification = (message, type) => {
    console.log("[App.js:dispatchNotification] Dispatching:", {
      message,
      type,
    });
    window.dispatchEvent(
      new CustomEvent("showMessage", { detail: { message, type } }),
    );
  };

  if (triggeringElement && triggeringElement.id === "login-form") {
    console.log("[App.js:afterOnLoad] Handling login-form response.");
    // Zawsze próbuj zapobiec domyślnej podmianie dla formularza logowania,
    // ponieważ sami zarządzamy komunikatami i przekierowaniem.
    if (evt.detail.hasOwnProperty("shouldSwap")) {
      evt.detail.shouldSwap = false;
      console.log(
        "[App.js:afterOnLoad] Set evt.detail.shouldSwap = false for login-form.",
      );
    }
    // Dodatkowo, wyczyśćmy target, na wypadek gdyby `shouldSwap` nie zadziałało idealnie.
    if (targetElement) {
      targetElement.innerHTML = "";
      console.log(
        "[App.js:afterOnLoad] Cleared targetElement for login-form:",
        targetElement.id,
      );
    }

    if (evt.detail.successful && xhr.responseText) {
      try {
        const response = JSON.parse(xhr.responseText);
        if (response.token) {
          console.log("[App.js:afterOnLoad] Login successful, token received.");
          localStorage.setItem("jwtToken", response.token);
          window.dispatchEvent(
            new CustomEvent("authChangedClient", {
              detail: { isAuthenticated: true },
            }),
          );
          dispatchNotification("Zalogowano pomyślnie!", "success");

          setTimeout(() => {
            if (window.htmx) {
              console.log("[App.js:afterOnLoad] Redirecting to /moje-konto");
              window.htmx.ajax("GET", "/htmx/moje-konto", {
                target: "#content",
                swap: "innerHTML",
                pushUrl: "/moje-konto",
              });
            }
          }, 1500);
          return; // Zakończ przetwarzanie
        } else if (response.message) {
          console.log(
            "[App.js:afterOnLoad] Login failed (server message):",
            response.message,
          );
          dispatchNotification(response.message, "error");
        } else {
          console.log(
            "[App.js:afterOnLoad] Login failed (unknown server response structure).",
          );
          dispatchNotification(
            "Logowanie nie powiodło się. Nieprawidłowa odpowiedź serwera.",
            "error",
          );
        }
      } catch (e) {
        console.error(
          "[App.js:afterOnLoad] Error parsing login response JSON:",
          e,
          xhr.responseText,
        );
        dispatchNotification("Błąd przetwarzania odpowiedzi serwera.", "error");
      }
    } else if (!evt.detail.successful) {
      // Błąd sieciowy lub status HTTP błędu (np. 400, 401, 422)
      let errorMessage = "Logowanie nie powiodło się. Spróbuj ponownie.";
      if (xhr.responseText) {
        try {
          const errorResponse = JSON.parse(xhr.responseText);
          if (errorResponse.message) {
            errorMessage = errorResponse.message;
          } else if (xhr.status === 400 || xhr.status === 401) {
            // 401 Unauthorized
            errorMessage = "Nieprawidłowy email lub hasło.";
          } else if (xhr.status === 422) {
            errorMessage = "Nieprawidłowe dane. Sprawdź formularz.";
          }
        } catch (e) {
          if (xhr.status === 400 || xhr.status === 401) {
            errorMessage = "Nieprawidłowy email lub hasło.";
          }
          console.error(
            "[App.js:afterOnLoad] Error parsing login error JSON:",
            e,
            xhr.responseText,
          );
        }
      } else if (xhr.status === 0) {
        errorMessage = "Błąd sieci. Sprawdź połączenie.";
      }
      console.log(
        "[App.js:afterOnLoad] Login failed (network/server error status " +
          xhr.status +
          "):",
        errorMessage,
      );
      dispatchNotification(errorMessage, "error");
    }
    return; // Zawsze return dla login-form, bo obsłużyliśmy go tutaj.
  } else if (
    triggeringElement &&
    triggeringElement.id === "registration-form"
  ) {
    console.log("[App.js:afterOnLoad] Handling registration-form response.");
    if (evt.detail.hasOwnProperty("shouldSwap")) {
      evt.detail.shouldSwap = false;
      console.log(
        "[App.js:afterOnLoad] Set evt.detail.shouldSwap = false for registration-form.",
      );
    }
    if (targetElement) {
      targetElement.innerHTML = "";
      console.log(
        "[App.js:afterOnLoad] Cleared targetElement for registration-form:",
        targetElement.id,
      );
    }

    if (evt.detail.successful && xhr.status === 201) {
      console.log("[App.js:afterOnLoad] Registration successful.");
      dispatchNotification(
        "Rejestracja pomyślna! Możesz się teraz zalogować.",
        "success",
      );
      if (triggeringElement) triggeringElement.reset();
    } else if (!evt.detail.successful && xhr.responseText) {
      let errorMessage = "Rejestracja nie powiodła się.";
      try {
        const errorResponse = JSON.parse(xhr.responseText);
        if (errorResponse.message) {
          errorMessage = errorResponse.message;
        } else if (errorResponse.errors) {
          const firstErrorField = Object.keys(errorResponse.errors)[0];
          if (
            firstErrorField &&
            Array.isArray(errorResponse.errors[firstErrorField]) &&
            errorResponse.errors[firstErrorField].length > 0
          ) {
            errorMessage = errorResponse.errors[firstErrorField][0];
          }
        } else if (xhr.status === 422) {
          errorMessage = "Nieprawidłowe dane. Sprawdź formularz.";
        }
      } catch (e) {
        console.error(
          "[App.js:afterOnLoad] Error parsing registration error JSON:",
          e,
          xhr.responseText,
        );
      }
      console.log("[App.js:afterOnLoad] Registration failed:", errorMessage);
      dispatchNotification(errorMessage, "error");
    }
    return; // Zawsze return dla registration-form.
  }
  // Jeśli to nie jest ani formularz logowania, ani rejestracji, pozwól HTMX działać domyślnie
  // lub dodaj inną specyficzną logikę.
});
