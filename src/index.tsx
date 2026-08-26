/* @refresh reload */
import { ErrorBoundary } from "solid-js";
import { render } from "solid-js/web";
import App from "./App";
import "./index.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Root element not found");
}

render(
  () => (
    <ErrorBoundary
      fallback={(error) => (
        <pre class="m-6 whitespace-pre-wrap rounded-lg border border-red-800 bg-red-950 p-4 text-red-200">
          Graphloom не смог загрузить интерфейс: {String(error)}
        </pre>
      )}
    >
      <App />
    </ErrorBoundary>
  ),
  root,
);
