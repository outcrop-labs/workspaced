(function () {
  if (window.__workspaced) return;
  window.__workspaced = true;
  const boot = window.__WORKSPACED_BOOT || {};
  const SENTINEL = boot.sentinel || "https://workspaced.invalid/rail";
  const suffixes = boot.suffixes || [];

  // Google pages get no IPC. Navigating to the sentinel is cancelled by the
  // shell, which reads the action out of the URL instead.
  function signal(action, payload) {
    location.href = SENTINEL + "?a=" + encodeURIComponent(action) + "&p=" + encodeURIComponent(JSON.stringify(payload || {}));
  }

  // A sleeping account's feed view: report the unread count and the newest
  // messages, then stay quiet. The shell reloads it to poll again. WebKit
  // shows raw XML through its own viewer, and the page's Trusted Types
  // policy refuses DOMParser, so the feed is fetched and its few fields are
  // read straight out of the text.
  if (location.host === "mail.google.com" && /\/feed\/atom/.test(location.pathname)) {
    const decode = function (value) {
      return value
        .replace(/&lt;/g, "<")
        .replace(/&gt;/g, ">")
        .replace(/&quot;/g, '"')
        .replace(/&#39;|&apos;/g, "'")
        .replace(/&#(\d+);/g, function (_, code) {
          return String.fromCharCode(Number(code));
        })
        .replace(/&amp;/g, "&")
        .trim();
    };
    const field = function (xml, tag) {
      const match = new RegExp("<" + tag + "(?:\\s[^>]*)?>([\\s\\S]*?)</" + tag + ">").exec(xml);
      return match ? decode(match[1]) : "";
    };
    fetch(location.href, { credentials: "include", cache: "no-store" })
      .then(function (response) {
        if (!response.ok) throw new Error("status " + response.status);
        return response.text();
      })
      .then(function (xml) {
        const count = /<fullcount>(\d+)<\/fullcount>/.exec(xml);
        if (!count) throw new Error("no feed in the response");
        const head = xml.split("<entry>")[0];
        const address = field(head, "title").match(/\S+@\S+/);
        const entries = (xml.match(/<entry>[\s\S]*?<\/entry>/g) || []).slice(0, 10).map(function (entry) {
          return {
            id: field(entry, "id"),
            title: field(entry, "title").slice(0, 120),
            author: field(entry, "name").slice(0, 60),
          };
        });
        signal("poll", { count: Number(count[1]), email: address ? address[0] : null, entries: entries });
      })
      .catch(function (reason) {
        signal("poll-failed", { reason: String(reason).slice(0, 120) });
      });
    return;
  }

  // Omarchy styles. The shell sends new ones when the theme changes.
  let styles = boot.styles || { css: "" };
  function paint() {
    if (!/html/.test(document.contentType || "text/html")) return;
    const root = document.documentElement;
    if (!root) return;
    let sheet = document.getElementById("workspaced-styles");
    if (!styles.css) {
      if (sheet) sheet.remove();
      return;
    }
    if (!sheet) {
      sheet = document.createElement("style");
      sheet.id = "workspaced-styles";
    }
    if (sheet.textContent !== styles.css) sheet.textContent = styles.css;
    // Last in the head, so it follows the page's own sheets.
    const parent = document.head || root;
    if (parent.lastElementChild !== sheet) parent.appendChild(sheet);
  }
  Object.defineProperty(window, "__workspacedStyles", {
    value: function (next) {
      styles = next || { css: "" };
      paint();
    },
  });

  // Contrast guard. Some text and panels are painted with literal greys,
  // like the colours of a Gmail theme or Keep's white panels, which the
  // palette leaves unreadable or glaring. Grey text too close to the
  // background takes the theme's text colours, and on dark themes near-white
  // panels take its surface colour. Anything coloured keeps its colour.
  const fixed = new WeakSet();
  function channels(value) {
    const m = /rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?/.exec(value || "");
    return m ? [Number(m[1]), Number(m[2]), Number(m[3]), m[4] === undefined ? 1 : Number(m[4])] : null;
  }
  function grey(c) {
    return Math.max(c[0], c[1], c[2]) - Math.min(c[0], c[1], c[2]) <= 40;
  }
  function lightness(c) {
    return (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) / 255;
  }
  // Whether the nearest painted background behind an element is light, like
  // a coloured chip or a highlighted item. Dark text there is readable.
  function onLight(element) {
    for (let e = element, i = 0; e && e.nodeType === 1 && i < 10; e = e.parentElement, i++) {
      const b = channels(getComputedStyle(e).backgroundColor);
      if (b && b[3] >= 0.5) return lightness(b) > 0.55;
    }
    return false;
  }
  function guard(element) {
    const text = styles.text;
    if (!text || fixed.has(element) || !element.style) return;
    fixed.add(element);
    const style = getComputedStyle(element);
    const c = channels(style.color);
    if (c && c[3] >= 0.2 && grey(c) && !onLight(element)) {
      const light = lightness(c);
      if (text.dark ? light < 0.36 : light > 0.66) {
        const strong = text.dark ? light < 0.2 : light > 0.82;
        element.style.setProperty("color", strong ? text.strong : text.soft, "important");
      }
    }
    const b = channels(style.backgroundColor);
    if (text.dark && b && b[3] >= 0.5 && grey(b) && lightness(b) > 0.8) {
      element.style.setProperty("background-color", text.surface, "important");
    }
  }
  // Each root gets a cursor that resumes on the next pass, so big pages are
  // covered in steady batches instead of stalling on their first few
  // thousand elements.
  const walkers = [];
  let scheduled = false;
  function sweep() {
    scheduled = false;
    let budget = 3000;
    while (walkers.length && budget > 0) {
      const node = walkers[0].nextNode();
      if (!node) {
        walkers.shift();
        continue;
      }
      guard(node);
      budget--;
    }
    if (walkers.length) schedule();
  }
  function schedule() {
    if (scheduled) return;
    scheduled = true;
    // A timer, not requestIdleCallback: busy pages like Gmail may never idle.
    window.setTimeout(sweep, 60);
  }
  function enqueue(root) {
    if (!root.isConnected) return;
    guard(root);
    walkers.push(document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT));
  }
  let watching = false;
  function watch() {
    if (watching || !document.body || !styles.text) return;
    watching = true;
    enqueue(document.body);
    schedule();
    new MutationObserver(function (records) {
      if (!styles.text) return;
      for (const record of records) {
        for (const node of record.addedNodes) if (node.nodeType === 1) enqueue(node);
        // A class change can restyle an element. Let it be checked again.
        if (record.type === "attributes" && record.target.nodeType === 1) {
          fixed.delete(record.target);
          guard(record.target);
        }
      }
      if (walkers.length) schedule();
    }).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["class"] });
  }

  // A timer rather than load events. Each pass puts the sheet back if the
  // page dropped or reordered it, and starts the guard once there is a body.
  function upkeep() {
    // Each step guarded, so a page that rejects one cannot stop the other.
    try {
      paint();
    } catch (_) {}
    try {
      watch();
    } catch (_) {}
    window.setTimeout(upkeep, document.readyState === "complete" ? 3000 : 300);
  }
  upkeep();

  function isGoogle(url) {
    try {
      const host = new URL(url, location.href).hostname.replace(/\.$/, "").toLowerCase();
      if (/(^|\.)google\.(com|[a-z]{2}|(co|com|org|net|ac|gov)\.[a-z]{2})$/.test(host)) return true;
      return suffixes.some(function (suffix) {
        return host === suffix || host.endsWith("." + suffix);
      });
    } catch (_) {
      return true;
    }
  }

  document.addEventListener("click", function (event) {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const link = event.target && event.target.closest ? event.target.closest("a[href]") : null;
    if (!link) return;
    const href = link.href;
    if (!/^(https?|mailto):/i.test(href) || href.startsWith(SENTINEL)) return;
    if (isGoogle(href)) return;
    event.preventDefault();
    event.stopPropagation();
    signal("external", { url: href });
  }, true);

  // Browser keys the webview does not provide on its own.
  document.addEventListener("keydown", function (event) {
    if (event.metaKey || event.shiftKey) return;
    const ctrl = event.ctrlKey && !event.altKey;
    const alt = event.altKey && !event.ctrlKey;
    let handled = true;
    if (event.key === "F5" || (ctrl && event.key === "r")) location.reload();
    else if (alt && event.key === "ArrowLeft") history.back();
    else if (alt && event.key === "ArrowRight") history.forward();
    else if (ctrl && event.key === ",") signal("settings");
    else if (ctrl && event.key === "w") signal("close-tab");
    else handled = false;
    if (handled) {
      event.preventDefault();
      event.stopPropagation();
    }
  }, true);

  document.addEventListener("keydown", function (event) {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    let action;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      action = ["cycle", { delta: event.key === "ArrowDown" ? 1 : -1 }];
    } else if (event.key >= "1" && event.key <= "9") {
      action = ["focus", { index: Number(event.key) - 1 }];
    } else {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    signal(action[0], action[1]);
  }, true);
})();
