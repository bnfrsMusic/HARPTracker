const { invoke } = window.__TAURI__.core;

//Dynamic fields

let utcMsg;
let dateMsg;

async function settings_but() {
  await invoke("settings");
}

async function utc() {
  //Make this set the text content to the current UTC time
  utcMsg.textContent = await invoke("utc");
}

async function date() {
  dateMsg.textContent = await invoke("date");
}

async function frame() {
  utc();
}

window.addEventListener("DOMContentLoaded", () => {
  utcMsg = document.querySelector("#utc-msg");
  dateMsg = document.querySelector("#date-msg");
  setInterval(frame, 100);
  date();

  document.querySelector("sett-button").addEventListener("submit", (e) => {
    e.preventDefault();
    settings_but();
  });
});
