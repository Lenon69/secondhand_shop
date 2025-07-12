/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.rs", // Skanuje pliki Rust w poszukiwaniu klas
    "./static/**/*.html", // Skanuje pliki HTML
    "./static/**/*.js", // Skanuje pliki JS
    "./src/*.rs", // Skanuje pliki Rust w poszukiwaniu klas
    "./static/*.html", // Skanuje pliki HTML
    "./static/*.js", // Skanuje pliki JS
  ],
  theme: {
    screens: {
      sm: "640px",
      // => @media (min-width: 640px) { ... }

      md: "768px",
      // => @media (min-width: 768px) { ... }

      // ZMIANA: Zwiększamy próg 'lg' z 1024px do 1280px
      // To sprawi, że layout desktopowy "załapie się" nawet na ekranie
      // 1920px ze skalowaniem 150% (1920 / 1.5 = 1280px)
      lg: "1280px",
      // => @media (min-width: 1280px) { ... }

      xl: "1440px", // Zwiększony próg dla jeszcze większych ekranów
      // => @media (min-width: 1440px) { ... }

      "2xl": "1600px", // Zwiększony próg dla bardzo dużych monitorów
      // => @media (min-width: 1600px) { ... }
    },
    extend: {
      // Tutaj możesz dodać inne rozszerzenia motywu, np. kolory
    },
  },
  plugins: [],
};
