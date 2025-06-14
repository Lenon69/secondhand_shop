function () {
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
);
