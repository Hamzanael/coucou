// The compose box: a small normal window for typing, opened by the island (the
// Ask tab, a reply to a session). Enter sends the text back to the island; Esc
// or clicking elsewhere puts it away.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./compose.css";

const input = document.getElementById("compose-input") as HTMLInputElement;
let target = "";

void listen<{ target: string; placeholder: string }>("compose-open", ({ payload }) => {
  target = payload.target;
  input.placeholder = payload.placeholder;
  input.value = "";
  window.setTimeout(() => input.focus(), 30);
});

input.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && input.value.trim()) {
    e.preventDefault();
    void invoke("compose_submit", { target, text: input.value.trim() });
  } else if (e.key === "Escape") {
    void invoke("compose_cancel");
  }
});

// Clicking anywhere else closes it, like a menu.
window.addEventListener("blur", () => void invoke("compose_cancel"));
