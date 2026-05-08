import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import WorkspaceSelect from "./pages/WorkspaceSelect";
import ProjectDashboard from "./pages/ProjectDashboard";
import "./styles/globals.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<WorkspaceSelect />} />
        <Route path="/project" element={<ProjectDashboard />} />
      </Routes>
    </BrowserRouter>
  </React.StrictMode>,
);
