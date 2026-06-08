const { invoke } = window.__TAURI__.core;

const list = document.getElementById("profile-list");
const dlgCreate = document.getElementById("dlg-create");
const dlgDelete = document.getElementById("dlg-delete");
const inputName = document.getElementById("input-name");
const colorOptions = document.getElementById("color-options");
const deleteName = document.getElementById("delete-name");

let selectedColor = "#7c3aed";
let deleteTarget = null;
let launching = false;

// --- Render ---

async function refresh() {
  const profiles = await invoke("list_profiles");
  const running = await invoke("get_running");

  const activeCount = running.length;
  document.getElementById("profile-count").textContent =
    `${profiles.length} perfil${profiles.length !== 1 ? "es" : ""} \u00B7 ${activeCount} activo${activeCount !== 1 ? "s" : ""}`;

  list.innerHTML = "";
  if (profiles.length === 0) {
    list.innerHTML = `
      <div class="empty">
        <p>Sin perfiles</p>
        <small>Crea uno con el boton + para empezar</small>
      </div>`;
    return;
  }

  for (const p of profiles) {
    const isRunning = running.includes(p.id);
    const card = document.createElement("div");
    card.className = "card" + (isRunning ? " active" : "");

    card.innerHTML = `
      <div class="card-header">
        <span class="card-dot" style="background:${p.color}"></span>
        <span class="card-name">${esc(p.name)}</span>
        ${isRunning ? '<span class="card-badge">Activo</span>' : ''}
      </div>
      <span class="card-status ${isRunning ? "running" : ""}">
        <span class="dot"></span>
        ${isRunning ? "En ejecucion" : "Detenido"}
      </span>
      <div class="card-actions">
        <button class="btn-launch">${isRunning ? "Reabrir" : "Abrir"}</button>
        <button class="btn-delete" ${isRunning ? 'disabled title="Cierra Claude antes de eliminar"' : ''}>Eliminar</button>
      </div>
    `;

    const btnLaunch = card.querySelector(".btn-launch");
    const btnDelete = card.querySelector(".btn-delete");

    btnLaunch.addEventListener("click", () => launchProfile(p.id, btnLaunch));
    if (!isRunning) {
      btnDelete.addEventListener("click", () => confirmDelete(p));
    }

    list.appendChild(card);
  }
}

// --- Actions ---

async function launchProfile(id, btn) {
  if (launching) return;
  launching = true;

  btn.textContent = "Iniciando...";
  btn.classList.add("loading");
  document.querySelectorAll(".btn-launch").forEach(b => b.disabled = true);

  try {
    await invoke("launch_profile", { id });
  } catch (e) {
    alert("Error al lanzar: " + e);
  }

  launching = false;
  document.querySelectorAll(".btn-launch").forEach(b => b.disabled = false);
  await refresh();
}

function confirmDelete(profile) {
  deleteTarget = profile.id;
  deleteName.textContent = profile.name;
  dlgDelete.showModal();
}

async function doDelete() {
  if (!deleteTarget) return;
  try {
    await invoke("delete_profile", { id: deleteTarget });
  } catch (e) {
    alert("Error al eliminar: " + e);
  }
  deleteTarget = null;
  dlgDelete.close();
  await refresh();
}

async function createProfile(name, color) {
  try {
    await invoke("create_profile", { name, color });
  } catch (e) {
    alert("Error al crear: " + e);
  }
  await refresh();
}

function esc(s) {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}

// --- Events ---

document.getElementById("btn-add").addEventListener("click", () => {
  inputName.value = "";
  selectedColor = "#7c3aed";
  colorOptions.querySelectorAll(".color-opt").forEach(b =>
    b.classList.toggle("selected", b.dataset.color === selectedColor)
  );
  dlgCreate.showModal();
  inputName.focus();
});

document.getElementById("form-create").addEventListener("submit", async (e) => {
  e.preventDefault();
  const name = inputName.value.trim();
  if (!name) return;
  dlgCreate.close();
  await createProfile(name, selectedColor);
});

colorOptions.addEventListener("click", (e) => {
  const btn = e.target.closest(".color-opt");
  if (!btn) return;
  selectedColor = btn.dataset.color;
  colorOptions.querySelectorAll(".color-opt").forEach(b =>
    b.classList.toggle("selected", b === btn)
  );
});

document.getElementById("btn-cancel-create").addEventListener("click", () => dlgCreate.close());
document.getElementById("btn-cancel-delete").addEventListener("click", () => { deleteTarget = null; dlgDelete.close(); });
document.getElementById("btn-confirm-delete").addEventListener("click", doDelete);

// Refresh every 3s
refresh();
setInterval(refresh, 3000);
