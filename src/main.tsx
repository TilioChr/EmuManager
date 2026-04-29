import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import DebugWindow from "./components/DebugWindow";
import "./styles.css";

const RootComponent = window.location.hash === "#/debug" ? DebugWindow : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RootComponent />
  </React.StrictMode>
);
