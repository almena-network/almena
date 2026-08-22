/**
 * Where the interface starts.
 *
 * The i18n import is first and it is not decorative: it settles which language is in use
 * before any component renders, so that nothing is ever drawn showing a key.
 */

import React from "react";
import ReactDOM from "react-dom/client";

import App from "@/App";
import "@/i18n";

const root = document.getElementById("root");

if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
