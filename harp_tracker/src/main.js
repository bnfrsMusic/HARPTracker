const { invoke } = window.__TAURI__.core;

let greetInputEl;
let greetMsgEl;
let utcMsg;

async function greet() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  greetMsgEl.textContent = await invoke("greet", { name: greetInputEl.value });
}

async function utc() {
  //Make this set the text content to the current UTC time
  utcMsg.textContent = await invoke("utc");
}

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  utcMsg = document.querySelector("#utc-msg");
  document.querySelector("#greet-form").addEventListener("submit", (e) => {
    e.preventDefault();
    greet();
  });
  utc();
  setInterval(utc, 1);
});
