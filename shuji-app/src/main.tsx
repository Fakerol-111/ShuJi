import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import WorkspaceSelect from "./pages/WorkspaceSelect";
import ProjectDashboard from "./pages/ProjectDashboard";
import LogsPage from "./pages/LogsPage";
import SetupPage from "./pages/SetupPage";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { getCodeTheme } from "./constants";
import "./styles/globals.css";

// Apply saved code theme
document.documentElement.setAttribute("data-code-theme", getCodeTheme());

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("root element not found");
ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <BrowserRouter>
      <ErrorBoundary>
        <Routes>
          <Route path="/" element={<WorkspaceSelect />} />
          <Route path="/project" element={<ProjectDashboard />} />
          <Route path="/setup" element={<SetupPage />} />
          <Route path="/logs" element={<LogsPage />} />
        </Routes>
      </ErrorBoundary>
    </BrowserRouter>
  </React.StrictMode>,
);
