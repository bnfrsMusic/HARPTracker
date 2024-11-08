const { invoke } = window.__TAURI__.core;

//Dynamic fields
let utcMsg;
let dateMsg;
let ub_id;

//------------------------Main Run Functions------------------------

async function init() {
  utcMsg = document.querySelector("#utc-msg");
  dateMsg = document.querySelector("#date-msg");
  ub_id = document.querySelector("#ubiquiti-id");

  //sets the date
  date();
  settings();

  //called every 1 millisecond to update all text and stuff
  setInterval(frame, 1);
}
async function frame() {
  utc();
}

//------------------------Functions------------------------
async function settings() {
  document.querySelector("#sett").addEventListener("click", (e) => {
    e.preventDefault();

    // Toggle the sidebar
    toggle_display("settSidebar");
  });
}

async function utc() {
  utcMsg.textContent = await invoke("utc");
}

async function date() {
  dateMsg.textContent = await invoke("date");
}

//------------------------Helper Functions------------------------

function toggle_display(obj) {
  if (document.querySelector("#" + obj).style.display != "none") {
    document.getElementById(obj).style.display = "none";
  } else {
    document.getElementById(obj).style.display = "flex";
  }
}

//------------------------Initialize------------------------

window.addEventListener("DOMContentLoaded", () => {
  init();
});
