import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import ProjectDashboard from "./pages/ProjectDashboard";
import LogsPage from "./pages/LogsPage";
import SetupPage from "./pages/SetupPage";
import { getCodeTheme } from "./constants";
import "./styles/globals.css";

// Apply saved code theme
document.documentElement.setAttribute("data-code-theme", getCodeTheme());

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("root element not found");
ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<ProjectDashboard />} />
        <Route path="/setup" element={<SetupPage />} />
        <Route path="/logs" element={<LogsPage />} />
      </Routes>
    </BrowserRouter>
  </React.StrictMode>,
);
