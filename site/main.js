// Instalator școală: pagina de prezentare. Fără biblioteci; pagina se citește și fără script.
document.documentElement.classList.add("js");

const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const sleep = (ms) => new Promise((r) => setTimeout(r, reduced ? 0 : ms));

/* Tema deschisă / întunecată; alegerea se ține minte. */
(function theme() {
  const button = $("#theme");
  if (!button) return;
  button.addEventListener("click", () => {
    const next = document.documentElement.dataset.theme === "light" ? "dark" : "light";
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem("tema", next);
    } catch {
      // Fără localStorage tema se schimbă doar pe pagina deschisă.
    }
  });
})();

/* Antetul: fereastra programului chiar instalează (pe jumătate de ecran, desigur). */
(function heroDemo() {
  const app = $("#hero-app");
  if (!app) return;
  const journal = $("#hero-journal code");
  const install = $('[data-act="install"]', app);
  const summary = $(".app-summary", app);
  const boxes = $$('input[type="checkbox"]', app);
  let running = false;
  let clock = new Date(2026, 9, 27, 9, 14, 2);

  const count = () => boxes.filter((b) => b.checked).length;
  const refresh = () => {
    install.textContent = `Instalează (${count()})`;
    install.disabled = running || count() === 0;
  };
  const stamp = (secs) => {
    clock = new Date(clock.getTime() + secs * 1000);
    return clock.toTimeString().slice(0, 8);
  };
  const log = (text, cls) => {
    const line = document.createElement("span");
    if (cls) line.className = cls;
    line.textContent = "\n" + text;
    journal.append(line);
    // Ultimele șapte rânduri rămân la vedere.
    while (journal.childNodes.length > 8) journal.childNodes[1].remove();
  };

  const commands = {
    Furnica: "dezarhivare … → C:\\Program Files\\Furnica",
    Cangurul: "dezarhivare … → C:\\Program Files\\Cangurul",
    "Sumatra PDF": '"…\\SumatraPDF-3.5.2-64-install.exe" "-s" "-all-users"',
    Audacity: '"…\\audacity-win-3.6.4-64bit.exe" "/VERYSILENT" "/NORESTART"',
    Shotcut: '"…\\shotcut-win64-241102.exe" "/S"',
  };
  const durations = { Furnica: 900, Cangurul: 800, "Sumatra PDF": 1100, Audacity: 1500, Shotcut: 1700 };

  async function run() {
    if (running || count() === 0) return;
    running = true;
    $$("button, input", app).forEach((el) => (el.disabled = true));
    summary.textContent = "";
    let done = 0;
    for (const box of boxes) {
      const row = box.closest(".app-row");
      const res = $(".res", row);
      res.className = "res";
      res.textContent = "";
      if (!box.checked) continue;
      const name = box.dataset.name;
      res.innerHTML = '<span class="spin" aria-hidden="true"></span>';
      log(`${stamp(1)} ${name}: ${commands[name]}`);
      await sleep(durations[name]);
      res.innerHTML = '<i class="ic ok" role="img" aria-label="reușit"></i>';
      log(`${stamp(Math.round(durations[name] / 60))} ${name}: Ok`, "ok");
      $(".st", row).textContent = `la zi (${row.querySelector("b").textContent.split(" ").pop()})`;
      box.checked = false;
      done++;
    }
    summary.textContent = done === 1 ? "1 instalat" : `${done} instalate`;
    running = false;
    $$("button, input", app).forEach((el) => (el.disabled = false));
    refresh();
  }

  app.addEventListener("change", refresh);
  app.addEventListener("click", (e) => {
    const act = e.target.closest("[data-act]")?.dataset.act;
    if (act === "all" || act === "none") {
      boxes.forEach((b) => (b.checked = act === "all"));
      refresh();
    } else if (act === "install") run();
  });
  refresh();
  // O dată singur, când antetul e la vedere: programul se vede lucrând.
  if (!reduced) setTimeout(run, 1600);
})();

/* Încearcă: aceleași reguli ca în program (select.rs). */
(function simulator() {
  const form = $("#sim-form");
  if (!form) return;
  const list = $("#sim-list");
  const PROGRAMS = [
    { name: "Furnica", version: "3.0.5", cat: "Educațional",
      win: [{ w: ["8", "10"], arch: "x86", kind: "zip" }], linux: [{ arch: "x64", kind: "deb" }] },
    { name: "Cangurul", version: "3.0.6", cat: "Educațional",
      win: [{ w: ["8", "10"], arch: "x86", kind: "zip" }], linux: [{ arch: "x64", kind: "deb" }] },
    { name: "Sumatra PDF", version: "3.5.2", cat: "Birou",
      win: [{ w: ["10"], arch: "x64", kind: "exe" }, { w: ["8", "10"], arch: "x86", kind: "exe" }], linux: [] },
    { name: "Audacity", version: "3.6.4", cat: "Multimedia",
      win: [{ w: ["10"], arch: "x64", kind: "exe" }], linux: [{ arch: "x64", kind: "appimage" }] },
  ];
  const OS = {
    win8: { label: "Windows 8.1", win: "8" },
    win10: { label: "Windows 10", win: "10" },
    win11: { label: "Windows 11", win: "10" },
    ubuntu: { label: "Ubuntu 24.04", linux: true },
    mint: { label: "Linux Mint 22", linux: true },
    antix: { label: "antiX 23", linux: true },
  };

  const segments = (v) => (v.match(/\d+/g) || []).map(Number);
  const compare = (a, b) => {
    const x = segments(a), y = segments(b);
    for (let i = 0; i < Math.max(x.length, y.length); i++) {
      const d = (x[i] || 0) - (y[i] || 0);
      if (d) return Math.sign(d);
    }
    return 0;
  };
  const choose = (p, os, arch) => {
    if (os.linux) {
      const fits = p.linux.filter((v) => v.arch === arch || v.arch === "x86");
      return ["deb", "appimage", "rpm"].map((k) => fits.find((v) => v.kind === k)).find(Boolean);
    }
    const fits = (a) => p.win.find((v) => v.w.includes(os.win) && v.arch === a);
    return fits(arch) || (arch === "x64" ? fits("x86") : undefined);
  };

  function render() {
    const data = new FormData(form);
    const os = OS[data.get("os")];
    const x86 = form.querySelector('input[value="x86"]');
    // Instalatorul pentru Linux există doar pe 64 de biți.
    x86.disabled = !!os.linux;
    if (os.linux && data.get("arch") === "x86") form.querySelector('input[value="x64"]').checked = true;
    const arch = new FormData(form).get("arch");
    const admin = data.get("admin") === "on";
    $("#sim-sys").textContent = os.linux
      ? `${os.label} · ${arch === "x64" ? "64" : "32"} biți · ${admin ? "root" : "fără root"}`
      : `${os.label === "Windows 8.1" ? "Windows 8" : "Windows 10"} · ${arch === "x64" ? "64" : "32"} biți · ${admin ? "Administrator" : "fără drepturi de administrator"}`;
    $("#sim-sys").insertAdjacentHTML("beforeend", admin ? ' <i class="ic ok" role="img" aria-label="da"></i>' : ' <i class="ic err" role="img" aria-label="nu"></i>');

    let checked = 0;
    let lastCat = "";
    list.innerHTML = "";
    for (const p of PROGRAMS) {
      const installed = data.get(p.name) || "";
      const variant = choose(p, os, arch);
      let label, check = false, off = false, bad = false;
      if (!variant) {
        label = `nu există variantă pentru ${os.linux ? "Linux" : os.win === "8" ? "Windows 8" : "Windows 10"} (${arch === "x64" ? "64" : "32"} biți)`;
        off = true;
      } else if (installed === "msi" && !os.linux) {
        label = "instalat altfel în C:\\Program Files\\Furnica; dezinstalați-l întâi";
        off = bad = true;
      } else if (!installed || installed === "msi") {
        label = "nu e instalat";
        check = true;
      } else if (compare(installed, p.version) < 0) {
        label = `actualizare ${installed} → ${p.version}`;
        check = true;
      } else {
        label = `la zi (${installed})`;
      }
      if (p.cat !== lastCat) {
        const cat = document.createElement("div");
        cat.className = "app-cat";
        cat.textContent = p.cat;
        list.append(cat);
        lastCat = p.cat;
      }
      const row = document.createElement("label");
      row.className = "app-row" + (off ? " off" : "") + (bad ? " bad" : "");
      row.innerHTML = `<input type="checkbox"${check ? " checked" : ""}${off ? " disabled" : ""}> <b></b> <span class="st"></span>`;
      $("b", row).textContent = `${p.name} ${p.version}`;
      $(".st", row).textContent = label;
      list.append(row);
      if (check) checked++;
    }
    const count = $("#sim-count");
    count.textContent = `Instalează (${checked})`;
    $(".app-summary", form.closest(".panel")).textContent = admin ? "" : "pornește-l ca administrator";
    list.onchange = () => {
      count.textContent = `Instalează (${$$("input:checked", list).length})`;
    };
  }
  form.addEventListener("input", render);
  render();
})();

/* Pentru administratori: lista se filtrează și se parcurge cu săgețile. */
(function guarantees() {
  const input = $("#g-filter");
  const items = $$("#g-list li");
  const empty = $(".g-empty");
  if (!input) return;
  const select = (li) => {
    items.forEach((i) => i.classList.toggle("sel", i === li));
  };
  items.forEach((li) => {
    li.addEventListener("mouseenter", () => matchMedia("(hover: hover)").matches && select(li));
    li.addEventListener("click", () => select(li));
    li.addEventListener("focus", () => select(li));
  });
  const fold = (s) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  input.addEventListener("input", () => {
    const q = fold(input.value.trim());
    const shown = items.filter((li) => {
      const hit = !q || fold(li.textContent).includes(q);
      li.hidden = !hit;
      return hit;
    });
    empty.hidden = shown.length > 0;
    if (shown.length && !shown.some((li) => li.classList.contains("sel"))) select(shown[0]);
  });
  input.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const shown = items.filter((li) => !li.hidden);
    const at = shown.findIndex((li) => li.classList.contains("sel"));
    const next = shown[(at + (e.key === "ArrowDown" ? 1 : -1) + shown.length) % shown.length];
    if (next) select(next);
  });
})();

/* Sisteme: filele Windows / Linux. */
(function tabs() {
  const tabs = $$('[role="tab"]');
  tabs.forEach((tab) =>
    tab.addEventListener("click", () => {
      tabs.forEach((t) => {
        const on = t === tab;
        t.setAttribute("aria-selected", on);
        document.getElementById(t.getAttribute("aria-controls")).hidden = !on;
      });
    }),
  );
})();

/* Intrările secțiunilor, o singură dată. */
(function reveal() {
  const els = $$(".reveal");
  if (!("IntersectionObserver" in window)) return els.forEach((el) => el.classList.add("in"));
  const io = new IntersectionObserver(
    (entries) => entries.forEach((e) => e.isIntersecting && (e.target.classList.add("in"), io.unobserve(e.target))),
    { rootMargin: "0px 0px -10% 0px" },
  );
  els.forEach((el) => io.observe(el));
})();

/* Butoanele de descărcare: fișierele și mărimile din ultimul release. */
(async function release() {
  try {
    const res = await fetch("https://api.github.com/repos/nazdravan/installer/releases/latest");
    if (!res.ok) return;
    const rel = await res.json();
    const mb = (n) => `· ${Math.round(n / 1e6)} MB`;
    for (const [os, id] of [["windows", "#dl-win"], ["linux", "#dl-lin"]]) {
      const asset = rel.assets.find((a) => a.name.endsWith(`-${os}.zip`));
      if (!asset) continue;
      $(id).href = asset.browser_download_url;
      $(`[data-size="${os}"]`).textContent = mb(asset.size);
    }
    const ver = $("#dl-ver");
    ver.textContent = `${ver.dataset.prefix} ${rel.tag_name.replace(/^v/, "")}`;
  } catch {
    // Fără rețea sau limită de cereri: rămân legăturile spre pagina release-ului.
  }
})();
