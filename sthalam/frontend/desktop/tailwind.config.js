/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{svelte,js,ts,jsx,tsx}",
  ],
  theme: {
    screens: {
      sm: "640px",
      md: "768px",
      lg: "1024px",
      xl: "1280px",
      "2xl": "1536px",
      "3xl": "1920px",
    },
    extend: {
      fontFamily: {
        sans: [
          '"Inter"',
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          '"Segoe UI"',
          "Roboto",
          '"Helvetica Neue"',
          "Arial",
          "sans-serif",
        ],
        jakarta: ['"Plus Jakarta Sans"', "sans-serif"],
      },
      fontWeight: {
        extralight: "200",
        light: "300",
        normal: "400",
        medium: "500",
        semibold: "600",
        bold: "700",
      },
      fontSize: {
        base: "16px",
        lg: "18px",
        xl: "20px",
        "2xl": "24px",
      },
      colors: {
        // Custom colors for the website builder theme
        mobile: {
          bgPrimary: "#0F0F14",
          bgSecondary: "#1A1A21",
          bgHighlight: "#25252E",
          textPrimary: "#FFFFFF",
          textSecondary: "#9CA3AF",
          textActive: "#A78BFA",
        },
        livnotePink: "#F472B6",
        disclaimerGray: "#9CA3AF",
        signupGray: "#4B5563",
        bgPrimary: "#1F242A",
        textActive: "#A78BFA",
        primarydark: "#1F242A",
        osvauld: {
          frameblack: "#1A1D23",
          iconblack: "#35353B",
          activeBorder: "#F472B6",
          sheffieldgrey: "#9CA3AF",
        },
      },
    },
  },
  plugins: [],
}
