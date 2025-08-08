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
      md: "768px",
      lg: "1280px",
      xl: "1440px", // Zwiększony próg dla jeszcze większych ekranów
      "2xl": "1600px", // Zwiększony próg dla bardzo dużych monitorów
    },
    extend: {
      colors: {
        "custom-footer-bg": "#ffece4",
        rosey: "#f88c8c",
        peachy: "#ffdcd4",
      },
    },
  },

  plugins: [],
};
