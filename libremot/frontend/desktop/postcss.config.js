// postcss.config.js
import postcssPresetEnv from "postcss-preset-env";
import postcssNesting from "postcss-nesting";
import postcssGapProperties from "postcss-gap-properties";
import autoprefixer from "autoprefixer";

export default {
  plugins: [
    postcssNesting(),
    postcssGapProperties(),
    postcssPresetEnv({
      stage: 3,
      features: {
        "nesting-rules": false, // Handled by postcss-nesting
        "gap-properties": false, // Handled by postcss-gap-properties
        "custom-properties": false, // Keep CSS custom properties as-is
      },
      browsers: [
        "> 1%",
        "last 2 versions",
        "not dead",
        "Chrome >= 58",
        "Safari >= 11",
        "Firefox >= 54",
        "Edge >= 79"
      ]
    }),
    autoprefixer({
      overrideBrowserslist: [
        "> 1%",
        "last 2 versions", 
        "not dead",
        "Chrome >= 58",
        "Safari >= 11",
        "Firefox >= 54",
        "Edge >= 79"
      ]
    })
  ],
};
