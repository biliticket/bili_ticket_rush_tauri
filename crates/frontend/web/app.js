let invoke = null;

let qrcodePollingInterval = null;
let currentTaskId = null;
let monitorStats = {
  attempts: 0,
  success: 0,
  failures: 0,
};

function showNotification(message, type = "info", duration = 5000) {
  const container = document.getElementById("notification-container");
  if (!container) return;

  const notification = document.createElement("div");
  notification.className = `notification ${type}`;

  const content = document.createElement("div");
  content.className = "notification-content";
  content.textContent = message;

  const closeBtn = document.createElement("button");
  closeBtn.className = "notification-close";
  closeBtn.innerHTML = "&times;";
  closeBtn.onclick = () => {
    notification.classList.add("hiding");
    setTimeout(() => notification.remove(), 300);
  };

  notification.appendChild(content);
  notification.appendChild(closeBtn);
  container.appendChild(notification);

  if (duration > 0) {
    setTimeout(() => {
      if (notification.parentNode) {
        notification.classList.add("hiding");
        setTimeout(() => notification.remove(), 300);
      }
    }, duration);
  }

  return notification;
}

function showAlert(message, type = "info") {
  showNotification(message, type, 5000);
}

function showSuccess(message) {
  showNotification(message, "success", 3000);
}

function showError(message) {
  showNotification(message, "error", 7000);
}

function showWarning(message) {
  showNotification(message, "warning", 5000);
}

function initializeEventListeners() {
  document.querySelectorAll(".nav-tab").forEach((tab) => {
    const tabName = tab.getAttribute("data-tab");
    if (tabName) {
      tab.addEventListener("click", (e) => {
        e.preventDefault();
        switchTab(tabName);
        return false;
      });
    }
  });

  const buttonIds = {
    "add-account-btn": showAddAccountModal,
    "reload-accounts-btn": reloadAccounts,
    "qrcode-login-btn": showQrcodeLoginModal,

    "start-grab-btn": startGrab,
    "stop-grab-btn": stopGrab,
    "refresh-monitor-btn": refreshMonitor
  };

  Object.keys(buttonIds).forEach((id) => {
    const element = document.getElementById(id);
    const handler = buttonIds[id];
    if (element && handler) {
      element.addEventListener("click", (e) => {
        e.preventDefault();
        handler();
      });
    }
  });

  document.querySelectorAll("form").forEach((form) => {
    form.addEventListener("submit", (e) => {
      e.preventDefault();
    });
  });
}

document.addEventListener("DOMContentLoaded", function () {
  console.log("DOM loaded, initializing application...");

  initializeTabSwitching();
  initializeEventListeners();

  let attempts = 0;
  const maxAttempts = 20;
  const checkInterval = 100;

  function checkTauriAvailability() {
    attempts++;
    console.log(
      `Checking for Tauri API (attempt ${attempts}/${maxAttempts})...`,
    );

    if (
      window.__TAURI__ &&
      window.__TAURI__.core &&
      window.__TAURI__.core.invoke
    ) {
      console.log("Tauri API found!");
      initializeApp();
    } else if (attempts < maxAttempts) {
      setTimeout(checkTauriAvailability, checkInterval);
    } else {
      console.log(
        "Tauri API not available after " +
          maxAttempts * checkInterval +
          "ms, using basic UI",
      );
      initializeBasicUI();
    }
  }

  checkTauriAvailability();
});

function initializeApp() {
  console.log("Tauri API available, initializing application...");

  try {
    invoke = window.__TAURI__.core.invoke;
    console.log("invoke function loaded successfully");

    init();
  } catch (error) {
    console.error("Failed to initialize Tauri API:", error);
    initializeBasicUI();
  }
}

function initializeBasicUI() {
  console.log("Initializing basic UI without Tauri functions...");

  const warning = document.createElement("div");
  warning.style.cssText = `
        position: fixed;
        top: 10px;
        left: 50%;
        transform: translateX(-50%);
        background: #ff6b6b;
        color: white;
        padding: 10px 20px;
        border-radius: 4px;
        z-index: 9999;
        font-size: 14px;
        text-align: center;
    `;
  warning.textContent = "Tauri API not available - some features may not work";
  document.body.appendChild(warning);

  updateUptime();

  setTimeout(function () {
    loadAccounts = function () {
      console.log("loadAccounts called (mock)");
      document.getElementById("accounts-loading").style.display = "none";
      document.getElementById("accounts-list").innerHTML =
        '<li style="padding: 20px; text-align: center; color: var(--text-secondary);">Tauri API not available</li>';
      document.getElementById("accounts-list").style.display = "block";
    };

    loadLogs = function () {
      console.log("loadLogs called (mock)");
      document.getElementById("logs-container").innerHTML =
        '<div class="log-entry">Tauri API not available - cannot load logs</div>';
    };

    loadAccounts();
  }, 100);
}

function initializeTabSwitching() {
  const navTabs = document.querySelectorAll(".nav-tab");
  console.log(`Found ${navTabs.length} nav tabs`);

  navTabs.forEach((tab) => {
    tab.style.cursor = "pointer";

    tab.onclick = function (e) {
      e.preventDefault();
      const tabName = this.dataset.tab;
      console.log("Tab clicked:", tabName);
      switchTab(tabName);
      return false;
    };
  });

  const homeTab = document.querySelector('[data-tab="home"]');
  if (homeTab && !homeTab.classList.contains("active")) {
    homeTab.classList.add("active");
  }
}

function showAddAccountModal() {
  const modal = document.getElementById("add-account-modal");
  if (modal) {
    modal.classList.add("active");
  } else {
    console.error("Add account modal not found");
  }
}

function closeAddProjectModal() {
  const modal = document.getElementById("add-project-modal");
  modal.classList.remove("active");

  document.getElementById("project-id").value = "";
  document.getElementById("project-name").value = "";
  document.getElementById("project-url").value = "";
}

function closeAddAccountModal() {
  const modal = document.getElementById("add-account-modal");
  if (modal) {
    modal.classList.remove("active");
  }

  const cookieInput = document.getElementById("account-cookie");
  if (cookieInput) {
    cookieInput.value = "";
  }
}

function showQrcodeLoginModal() {
  document.getElementById("qrcode-login-modal").classList.add("active");
  refreshQrcode();
}

function closeQrcodeModal() {
  document.getElementById("qrcode-login-modal").classList.remove("active");

  if (qrcodePollingInterval) {
    clearInterval(qrcodePollingInterval);
    qrcodePollingInterval = null;
  }
}

async function refreshQrcode() {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    const qrcodeData = await invoke("qrcode_login");

    if (qrcodeData && qrcodeData.url) {
      document.getElementById("qrcode-img").src = qrcodeData.url;

      startQrcodePolling(qrcodeData.key);
    } else {
      throw new Error("无法生成二维码");
    }
  } catch (error) {
    console.error("刷新二维码失败:", error);
    showError("生成二维码失败: " + error.message);
  }
}

function startQrcodePolling(qrcodeKey) {
  if (qrcodePollingInterval) {
    clearInterval(qrcodePollingInterval);
  }

  qrcodePollingInterval = setInterval(async () => {
    try {
      if (!invoke) {
        throw new Error("Tauri invoke function not available");
      }

      const result = await invoke("poll_qrcode_status", { key: qrcodeKey });

      if (result.status === "success") {
        clearInterval(qrcodePollingInterval);
        qrcodePollingInterval = null;

        if (result.cookie) {
          try {
            await invoke("add_account_by_cookie", { cookie: result.cookie });
            showSuccess("登录成功！账号已添加");
          } catch (error) {
            console.error("添加账号失败:", error);
            showError("登录成功但添加账号失败: " + error);
          }
        } else {
          showSuccess("登录成功！");
        }

        closeQrcodeModal();
        await reloadAccounts();
      } else if (result.status === "expired") {
        clearInterval(qrcodePollingInterval);
        qrcodePollingInterval = null;
        showWarning("二维码已过期，请刷新二维码");
      } else if (result.status === "error") {
        clearInterval(qrcodePollingInterval);
        qrcodePollingInterval = null;
        showError("登录失败: " + result.message);
      }
    } catch (error) {
      console.error("轮询二维码状态失败:", error);
    }
  }, 3000);
}

function showAddProjectModal() {
  document.getElementById("add-project-modal").classList.add("active");
}

async function submitAddAccount() {
  const cookie = document.getElementById("account-cookie").value;
  if (!cookie) {
    showWarning("请输入Cookie");
    return;
  }

  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }
    await invoke("add_account_by_cookie", { cookie });
    showSuccess("添加成功！");
    closeAddAccountModal();
    await reloadAccounts();
  } catch (error) {
    showError("添加失败: " + error);
  }
}

async function submitAddProject() {
  const projectId = document.getElementById("project-id").value;
  const projectName = document.getElementById("project-name").value;
  const projectUrl = document.getElementById("project-url").value;

  if (!projectId || !projectName || !projectUrl) {
    showWarning("请填写所有字段");
    return;
  }

  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    if (!/^\d+$/.test(projectId)) {
      showWarning("项目ID必须为数字");
      return;
    }

    if (
      !projectUrl.startsWith("http://") &&
      !projectUrl.startsWith("https://")
    ) {
      showWarning("请输入有效的URL（以http://或https://开头）");
      return;
    }

    await invoke("add_project", {
      id: projectId,
      name: projectName,
      url: projectUrl,
    });

    showSuccess("添加项目成功！");
    closeAddProjectModal();
    await loadProjects();
  } catch (error) {
    showError("添加失败: " + error);
  }
}

async function loadAccounts() {
  const loading = document.getElementById("accounts-loading");
  const list = document.getElementById("accounts-list");

  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    const accounts = await invoke("get_accounts");

    loading.style.display = "none";
    list.style.display = "block";
    list.innerHTML = "";

    if (accounts.length === 0) {
      list.innerHTML =
        '<li style="padding: 20px; text-align: center; color: var(--text-secondary);">暂无账号</li>';
      return;
    }

    accounts.forEach((account) => {
      const li = document.createElement("li");
      li.className = "account-item";
      li.innerHTML = `
                <div class="account-info">
                    <div class="account-name">${account.name}</div>
                    <div class="account-meta">UID: ${account.uid} | 等级: ${account.level} | ${account.vip_label}</div>
                </div>
                <div>
                    <label class="toggle-switch">
                        <input type="checkbox" ${account.is_active ? "checked" : ""} onchange="toggleAccountActive(${account.uid}, this.checked)">
                        <span class="toggle-slider"></span>
                    </label>
                    <button class="btn btn-danger" onclick="deleteAccount(${account.uid})">删除</button>
                </div>
            `;
      list.appendChild(li);
    });

    const countEl = document.getElementById("account-count");
    if (countEl) {
        countEl.textContent = accounts.length;
    }
  } catch (error) {
    console.error("Failed to load accounts:", error);
    loading.style.display = "none";
    list.style.display = "block";
    list.innerHTML = `<div style="padding: 20px; text-align: center; color: var(--error-color);">加载失败: ${error.message}</div>`;
  }
}

async function toggleAccountActive(uid, active) {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }
    await invoke("set_account_active", { uid, active });
  } catch (error) {
    showError("更新账号状态失败: " + error);
    await reloadAccounts();
  }
}

async function reloadAccounts() {
  document.getElementById("accounts-loading").style.display = "block";
  document.getElementById("accounts-list").style.display = "none";
  await loadAccounts();
}

async function deleteAccount(uid) {
  if (!confirm("确定要删除此账号吗？")) return;

  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }
    await invoke("delete_account_by_uid", { uid });
    await loadAccounts();
  } catch (error) {
    showError("删除失败: " + error);
  }
}

async function loadProjects() {
  const loading = document.getElementById("projects-loading");
  const list = document.getElementById("projects-list");

  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    const projects = await invoke("get_projects");

    loading.style.display = "none";
    list.style.display = "grid";
    list.innerHTML = "";

    if (!projects || projects.length === 0) {
      list.innerHTML =
        '<div style="padding: 40px; text-align: center; color: var(--text-secondary); grid-column: 1 / -1;">暂无项目</div>';
      return;
    }

    projects.forEach((project) => {
      const div = document.createElement("div");
      div.className = "project-card";
      div.innerHTML = `
                <div class="project-name">${project.name || "未命名项目"}</div>
                <div class="project-info">ID: ${project.id}</div>
                <div class="project-info" style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">${project.url || "无URL"}</div>
                <div style="margin-top: 12px; display: flex; gap: 8px;">
                    <button class="btn btn-primary" style="padding: 6px 12px; font-size: 12px;" onclick="selectProject('${project.id}')">选择</button>
                    <button class="btn btn-danger" style="padding: 6px 12px; font-size: 12px;" onclick="deleteProject('${project.id}')">删除</button>
                </div>
            `;
      list.appendChild(div);
    });
  } catch (error) {
    console.error("加载项目失败:", error);
    loading.style.display = "none";
    list.style.display = "grid";
    list.innerHTML = `<div style="padding: 40px; text-align: center; color: var(--error-color); grid-column: 1 / -1;">加载失败: ${error.message}</div>`;
  }
}

function addProject() {
  showAddProjectModal();
}

async function refreshProjects() {
  document.getElementById("projects-loading").style.display = "block";
  document.getElementById("projects-list").style.display = "none";
  await loadProjects();
}

async function selectProject(projectId) {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    await invoke("set_ticket_id", { ticketId: projectId });

    const accounts = await invoke("get_accounts");
    const activeAccount = accounts.find((a) => a.is_active);

    if (!activeAccount) {
      showWarning("请先激活一个账号");
      return;
    }

    showScreenTicketModal();

    const taskId = await invoke("get_ticket_info", {
      uid: activeAccount.uid,
      projectId: projectId,
    });

    console.log("获取项目详情任务ID:", taskId);

    const ticketInfo = await pollForTicketInfo(taskId);

    showScreenTicketSelector(ticketInfo);
  } catch (error) {
    showError("选择项目失败: " + error);
    closeScreenTicketModal();
  }
}

async function pollForTicketInfo(taskId) {
  const maxAttempts = 30;

  for (let i = 0; i < maxAttempts; i++) {
    try {
      const results = await invoke("poll_task_results");

      const result = results.find((r) => r.type === "GetTicketInfoResult");

      if (result) {
        if (result.success && result.ticket_info) {
          console.log("项目详情获取成功:", result.ticket_info);
          return result.ticket_info.data;
        } else if (!result.success) {
          throw new Error(result.message || "获取项目详情失败");
        }
      }

      await new Promise((resolve) => setTimeout(resolve, 500));
    } catch (error) {
      console.error("轮询错误:", error);
      throw error;
    }
  }

  throw new Error("获取项目详情超时");
}

function showScreenTicketModal() {
  const modal = document.getElementById("screen-ticket-modal");
  const loading = document.getElementById("screen-ticket-loading");
  const selector = document.getElementById("screen-ticket-selector");

  modal.classList.add("active");
  loading.style.display = "block";
  selector.style.display = "none";

  // 隐藏购票人类型选择框
  const buyerTypeGroup = document.querySelector(
    ".form-group:has(#buyer-type-select)",
  );
  if (buyerTypeGroup) {
    buyerTypeGroup.style.display = "none";
  }
}

function closeScreenTicketModal() {
  const modal = document.getElementById("screen-ticket-modal");
  const loading = document.getElementById("screen-ticket-loading");
  const selector = document.getElementById("screen-ticket-selector");

  modal.classList.remove("active");

  loading.style.display = "block";

  selector.style.display = "none";
  document.getElementById("screen-select").innerHTML = "";
  document.getElementById("ticket-select").innerHTML = "";

  document.getElementById("buyer-list").innerHTML = "";
  document.getElementById("buyer-list").style.display = "none";
  document.getElementById("buyer-loading").style.display = "none";
  document.getElementById("buyer-error").style.display = "none";

  document.getElementById("no-bind-name").value = "";
  document.getElementById("no-bind-tel").value = "";

  // 显示购票人类型选择框（下次打开时重新隐藏）
  const buyerTypeGroup = document.querySelector(
    ".form-group:has(#buyer-type-select)",
  );
  if (buyerTypeGroup) {
    buyerTypeGroup.style.display = "block";
  }
}

async function showScreenTicketSelector(ticketInfo) {
  const loading = document.getElementById("screen-ticket-loading");
  const selector = document.getElementById("screen-ticket-selector");
  const screenSelect = document.getElementById("screen-select");
  const ticketSelect = document.getElementById("ticket-select");

  loading.style.display = "none";
  selector.style.display = "block";

  const availableScreens = ticketInfo.screen_list.filter(
    (s) => s.clickable !== false,
  );

  if (availableScreens.length === 0) {
    showWarning("暂无可选场次");
    closeScreenTicketModal();
    return;
  }

  screenSelect.innerHTML = availableScreens
    .map(
      (s) =>
        `<option value="${s.id}">${s.name} (${new Date(s.start_time * 1000).toLocaleString()})</option>`,
    )
    .join("");

  window.currentTicketInfo = ticketInfo;

  screenSelect.onchange = function () {
    updateTicketList(parseInt(this.value));
  };

  updateTicketList(availableScreens[0].id);

  const idBind = ticketInfo.id_bind;
  console.log("项目实名制类型 id_bind:", idBind);

  const realNameSection = document.getElementById("real-name-buyer-section");
  const nonRealNameSection = document.getElementById(
    "non-real-name-buyer-section",
  );

  if (idBind === 0) {
    realNameSection.style.display = "none";
    nonRealNameSection.style.display = "block";
    showNotification("当前项目为非强实名制，请填写姓名和手机号", "info", 5000);
  } else if (idBind === 1 || idBind === 2) {
    realNameSection.style.display = "block";
    nonRealNameSection.style.display = "none";
    showNotification("当前项目为强实名制，请从购票人列表中选择", "info", 5000);
  } else {
    realNameSection.style.display = "block";
    nonRealNameSection.style.display = "none";
    console.warn("未知的实名制类型 id_bind:", idBind);
  }

  await loadBuyerInfo();
}

function updateTicketList(screenId) {
  const ticketSelect = document.getElementById("ticket-select");
  const ticketInfo = window.currentTicketInfo;

  if (!ticketInfo) return;

  const selectedScreen = ticketInfo.screen_list.find((s) => s.id === screenId);

  if (!selectedScreen || !selectedScreen.ticket_list) {
    ticketSelect.innerHTML = '<option value="">暂无票种</option>';
    return;
  }

  ticketSelect.innerHTML = selectedScreen.ticket_list
    .map((t) => {
      const price = (t.price / 100).toFixed(2);
      const status =
        t.sale_type === 1 ? "可售" : t.sale_type === 2 ? "售罄" : "未开售";
      return `<option value="${t.id}">${t.desc} - ¥${price} [${status}]</option>`;
    })
    .join("");
}

async function confirmScreenTicketSelection() {
  try {
    const screenId = document.getElementById("screen-select").value;
    const ticketId = document.getElementById("ticket-select").value;

    if (!screenId || !ticketId) {
      showWarning("请选择场次和票种");
      return;
    }

    // 根据当前显示的购票人部分确定购票人类型
    const realNameSection = document.getElementById("real-name-buyer-section");
    const nonRealNameSection = document.getElementById(
      "non-real-name-buyer-section",
    );

    let buyerType = "";
    if (realNameSection.style.display !== "none") {
      buyerType = "1";
    } else if (nonRealNameSection.style.display !== "none") {
      buyerType = "0";
    } else {
      showWarning("无法确定购票人类型");
      return;
    }

    if (buyerType === "1") {
      const selectedBuyers = getSelectedBuyers();
      console.log("选中的购票人数据:", JSON.stringify(selectedBuyers, null, 2));
      console.log("购票人数量:", selectedBuyers.length);

      if (selectedBuyers.length === 0) {
        showWarning("请至少选择一个购票人");
        return;
      }
    } else if (buyerType === "0") {
      const name = document.getElementById("no-bind-name").value.trim();
      const tel = document.getElementById("no-bind-tel").value.trim();

      if (!name || !tel) {
        showWarning("请填写非实名购票人的姓名和手机号");
        return;
      }

      // 验证手机号格式
      const phoneRegex = /^1[3-9]\d{9}$/;
      if (!phoneRegex.test(tel)) {
        showWarning("请输入有效的手机号");
        return;
      }
    }

    await invoke("set_selected_screen", {
      index: null,
      id: parseInt(screenId),
    });
    await invoke("set_selected_ticket", {
      id: parseInt(ticketId),
    });

    await invoke("set_buyer_type", { buyerType: parseInt(buyerType) });

    if (buyerType === "1") {
      const selectedBuyers = getSelectedBuyers();
      const validatedBuyers = selectedBuyers.map((buyer) => ({
        id: Number(buyer.id),
        uid: Number(buyer.uid) || 0,
        personal_id: String(buyer.personal_id || ""),
        name: String(buyer.name || ""),
        tel: String(buyer.tel || ""),
        id_type: Number(buyer.id_type) || 1,
        is_default: Number(buyer.is_default) || 0,
        id_card_front: String(buyer.id_card_front || ""),
        id_card_back: String(buyer.id_card_back || ""),
      }));

      console.log(
        "验证后的实名购票人数据:",
        JSON.stringify(validatedBuyers, null, 2),
      );

      await invoke("set_selected_buyer_list", { buyerList: validatedBuyers });

      await invoke("clear_no_bind_buyer_info");

      console.log(
        "已设置场次:",
        screenId,
        "票种:",
        ticketId,
        "实名购票人数量:",
        validatedBuyers.length,
      );
    } else if (buyerType === "0") {
      const name = document.getElementById("no-bind-name").value.trim();
      const tel = document.getElementById("no-bind-tel").value.trim();

      console.log("非实名购票人信息:", { name, tel });

      await invoke("set_no_bind_buyer_info", { name, tel });

      await invoke("set_selected_buyer_list", { buyerList: null });

      console.log(
        "已设置场次:",
        screenId,
        "票种:",
        ticketId,
        "非实名购票人:",
        name,
      );
    }

    showSuccess("设置成功");
    closeScreenTicketModal();
  } catch (error) {
    console.error("确认选择失败:", error);
    showError("设置失败: " + error);
  }
}

function onBuyerTypeChange() {
  // 此函数现在不再需要，因为购票人类型选择框已隐藏
  // 保留函数定义以避免调用错误
}

async function saveNoBindBuyerInfo() {
  try {
    const name = document.getElementById("no-bind-name").value.trim();
    const tel = document.getElementById("no-bind-tel").value.trim();

    if (!name || !tel) {
      showError("请填写姓名和手机号");
      return;
    }

    const phoneRegex = /^1[3-9]\d{9}$/;
    if (!phoneRegex.test(tel)) {
      showError("请输入有效的手机号");
      return;
    }

    await invoke("set_no_bind_buyer_info", { name, tel });

    showSuccess("非实名购票人信息保存成功！");
  } catch (error) {
    console.error("保存非实名购票人信息失败:", error);
    showError("保存失败: " + error);
  }
}

async function loadBuyerInfo() {
  const buyerLoading = document.getElementById("buyer-loading");
  const buyerList = document.getElementById("buyer-list");
  const buyerError = document.getElementById("buyer-error");

  try {
    buyerLoading.style.display = "block";
    buyerList.style.display = "none";
    buyerError.style.display = "none";

    const accounts = await invoke("get_accounts");
    const activeAccount = accounts.find((a) => a.is_active);

    if (!activeAccount) {
      throw new Error("请先激活一个账号");
    }

    const taskId = await invoke("get_buyer_info", {
      uid: activeAccount.uid,
    });

    if (!taskId || taskId.trim() === "") {
      throw new Error("获取任务ID失败，返回空值");
    }

    const buyerInfo = await pollForBuyerInfo(taskId);

    displayBuyerList(buyerInfo);

    buyerLoading.style.display = "none";
    buyerList.style.display = "block";
  } catch (error) {
    console.error("加载购票人信息失败:", error);
    console.error("错误堆栈:", error.stack);
    buyerLoading.style.display = "none";
    buyerError.style.display = "block";
    buyerError.textContent = "加载购票人失败: " + error.message;
  }
}

async function pollForBuyerInfo(taskId) {
  const maxAttempts = 30;
  for (let i = 0; i < maxAttempts; i++) {
    await new Promise((resolve) => setTimeout(resolve, 500));

    const results = await invoke("poll_task_results");

    const result = results.find((r) => r.task_id === taskId);

    if (result) {
      if (!result.success) {
        throw new Error(result.message || "获取购票人信息失败");
      }
      return result.buyer_info;
    }
  }
  throw new Error("获取购票人信息超时");
}

function displayBuyerList(buyerInfo) {
  const buyerList = document.getElementById("buyer-list");

  if (
    !buyerInfo ||
    !buyerInfo.data ||
    !buyerInfo.data.list ||
    buyerInfo.data.list.length === 0
  ) {
    buyerList.innerHTML =
      '<p style="color: #888; padding: 10px;">暂无购票人，请先在账号页面添加</p>';
    return;
  }

  const buyers = buyerInfo.data.list;

  buyerList.innerHTML = buyers
    .map(
      (buyer) => `
        <div class="buyer-item" style="display: flex; align-items: center; padding: 10px; border-bottom: 1px solid #eee;">
          <input
            type="checkbox"
            id="buyer-${buyer.id}"
            value="${buyer.id}"
            data-buyer='${encodeURIComponent(JSON.stringify(buyer))}'
            style="margin-right: 10px;"
          />
          <label for="buyer-${buyer.id}" style="flex: 1; cursor: pointer;">
            <strong>${buyer.name}</strong> - ${buyer.tel}
            ${buyer.is_default === 1 ? '<span style="color: #4CAF50; margin-left: 10px;">(默认)</span>' : ""}
          </label>
        </div>
      `,
    )
    .join("");

  if (buyers.length > 0) {
    const firstCheckbox = document.getElementById(`buyer-${buyers[0].id}`);
    if (firstCheckbox) {
      firstCheckbox.checked = true;
    }
  }
}

function getSelectedBuyers() {
  const checkboxes = document.querySelectorAll(
    '#buyer-list input[type="checkbox"]:checked',
  );
  const selectedBuyers = [];

  console.log("找到选中的复选框数量:", checkboxes.length);

  checkboxes.forEach((checkbox, index) => {
    try {
      const buyerJson = checkbox.getAttribute("data-buyer");
      console.log(`复选框 ${index + 1} 的原始数据:`, buyerJson);

      if (!buyerJson) {
        console.warn(`复选框 ${index + 1} 没有 data-buyer 属性`);
        return;
      }

      const decodedJson = decodeURIComponent(buyerJson);
      const buyerData = JSON.parse(decodedJson);
      console.log(`解析后的购票人 ${index + 1}:`, buyerData);

      if (!buyerData.id || !buyerData.name || !buyerData.tel) {
        console.warn(`购票人 ${index + 1} 缺少必需字段:`, buyerData);
      }

      selectedBuyers.push(buyerData);
    } catch (e) {
      console.error("解析购票人数据失败:", e);
      console.error("原始数据:", buyerJson);
      console.error("解码后数据:", decodedJson || "undefined");
    }
  });

  console.log("最终选中的购票人数组:", selectedBuyers);
  return selectedBuyers;
}

async function deleteProject(projectId) {
  if (!confirm("确定要删除此项目吗？")) return;
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    await invoke("delete_project", { id: projectId });
    showSuccess("删除项目成功");
    await loadProjects();
  } catch (error) {
    showError("删除失败: " + error);
  }
}

async function startGrab() {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    await invoke("set_grab_mode", { mode: 1 });
    const taskId = await invoke("start_grab_ticket");

    currentTaskId = taskId;

    document.getElementById("monitor-status").textContent = "运行中";
    document.getElementById("monitor-status").style.color =
      "var(--success-color)";

    showSuccess("开始抢票! 任务ID: " + taskId);
    await refreshMonitor();
  } catch (error) {
    console.error("启动抢票失败:", error);
    showError("启动失败: " + error);
  }
}

async function stopGrab() {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    if (currentTaskId) {
      try {
        await invoke("cancel_task", { taskId: currentTaskId });
        showSuccess("已取消抢票任务: " + currentTaskId);
      } catch (cancelError) {
        console.warn("取消任务失败:", cancelError);
        showWarning("取消任务失败，但已停止抢票模式: " + cancelError);
      }
      currentTaskId = null;
    }

    await invoke("set_grab_mode", { mode: 0 });
    document.getElementById("monitor-status").textContent = "已停止";
    document.getElementById("monitor-status").style.color =
      "var(--error-color)";

    showInfo("停止抢票");
  } catch (error) {
    showError("停止失败: " + error);
  }
}

async function refreshMonitor() {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    const state = await invoke("get_state");
    document.getElementById("monitor-status").textContent =
      state.running_status;

    const statusElem = document.getElementById("monitor-status");
    if (
      state.running_status.includes("运行") ||
      state.running_status.includes("抢票")
    ) {
      statusElem.style.color = "var(--success-color)";
      statusElem.style.fontWeight = "bold";
    } else if (state.running_status.includes("停止")) {
      statusElem.style.color = "var(--error-color)";
      statusElem.style.fontWeight = "normal";
    } else {
      statusElem.style.color = "var(--text-primary)";
      statusElem.style.fontWeight = "normal";
    }

    const stats = await invoke("get_monitor_stats");
    if (stats) {
      monitorStats.attempts = stats.attempts || 0;
      monitorStats.success = stats.success || 0;
      monitorStats.failures = stats.failures || 0;
      updateMonitorStats();
    }
  } catch (error) {
    console.error("刷新监控失败:", error);
  }
}

async function loadSettings() {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    const state = await invoke("get_state");
    document.getElementById("grab-mode").value = state.grab_mode || "0";
    document.getElementById("delay-time").value = state.status_delay || "2";
    document.getElementById("max-attempts").value =
      state.config?.max_attempts || "100";

    if (state.custom_config && state.custom_config.open_custom_ua) {
      document.getElementById("custom-ua").checked = true;
      document.getElementById("user-agent").value =
        state.custom_config.custom_ua || "";
    }

    if (state.push_config) {
      document.getElementById("enable-push").checked =
        state.push_config.enabled || false;
      document.getElementById("bark-token").value =
        state.push_config.bark_token || "";
      document.getElementById("pushplus-token").value =
        state.push_config.pushplus_token || "";
      document.getElementById("fangtang-token").value =
        state.push_config.fangtang_token || "";
      document.getElementById("dingtalk-token").value =
        state.push_config.dingtalk_token || "";
      document.getElementById("wechat-token").value =
        state.push_config.wechat_token || "";

      // 设置多选框状态
      if (state.push_config.enabled_methods) {
        document.getElementById("push-method-bark").checked =
          state.push_config.enabled_methods.includes("bark");
        document.getElementById("push-method-pushplus").checked =
          state.push_config.enabled_methods.includes("pushplus");
        document.getElementById("push-method-fangtang").checked =
          state.push_config.enabled_methods.includes("fangtang");
        document.getElementById("push-method-dingtalk").checked =
          state.push_config.enabled_methods.includes("dingtalk");
        document.getElementById("push-method-wechat").checked =
          state.push_config.enabled_methods.includes("wechat");
        document.getElementById("push-method-smtp").checked =
          state.push_config.enabled_methods.includes("smtp");
        document.getElementById("push-method-gotify").checked =
          state.push_config.enabled_methods.includes("gotify");
      } else {
        // 如果没有enabled_methods，默认全选
        document.getElementById("push-method-bark").checked = true;
        document.getElementById("push-method-pushplus").checked = true;
        document.getElementById("push-method-fangtang").checked = true;
        document.getElementById("push-method-dingtalk").checked = true;
        document.getElementById("push-method-wechat").checked = true;
        document.getElementById("push-method-smtp").checked = true;
        document.getElementById("push-method-gotify").checked = true;
      }

      // 更新推送设置的可见性
      updatePushSettingsVisibility();

      if (state.push_config.gotify_config) {
        document.getElementById("gotify-url").value =
          state.push_config.gotify_config.gotify_url || "";
        document.getElementById("gotify-token").value =
          state.push_config.gotify_config.gotify_token || "";
      }

      if (state.push_config.smtp_config) {
        document.getElementById("smtp-server").value =
          state.push_config.smtp_config.smtp_server || "";
        document.getElementById("smtp-port").value =
          state.push_config.smtp_config.smtp_port || "";
        document.getElementById("smtp-username").value =
          state.push_config.smtp_config.smtp_username || "";
        document.getElementById("smtp-password").value =
          state.push_config.smtp_config.smtp_password || "";
        document.getElementById("smtp-from").value =
          state.push_config.smtp_config.smtp_from || "";
        document.getElementById("smtp-to").value =
          state.push_config.smtp_config.smtp_to || "";
      }
    }
  } catch (error) {
    console.error("加载设置失败:", error);
  }
}

async function saveSettings() {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    const grabMode = document.getElementById("grab-mode").value;
    const delayTime = document.getElementById("delay-time").value;
    const maxAttempts = document.getElementById("max-attempts").value;
    const enablePush = document.getElementById("enable-push").checked;
    const barkToken = document.getElementById("bark-token").value;
    const pushplusToken = document.getElementById("pushplus-token").value;
    const fangtangToken = document.getElementById("fangtang-token").value;
    const dingtalkToken = document.getElementById("dingtalk-token").value;
    const wechatToken = document.getElementById("wechat-token").value;
    const gotifyUrl = document.getElementById("gotify-url").value;
    const gotifyToken = document.getElementById("gotify-token").value;
    const smtpServer = document.getElementById("smtp-server").value;
    const smtpPort = document.getElementById("smtp-port").value;
    const smtpUsername = document.getElementById("smtp-username").value;
    const smtpPassword = document.getElementById("smtp-password").value;
    const smtpFrom = document.getElementById("smtp-from").value;
    const smtpTo = document.getElementById("smtp-to").value;
    const customUa = document.getElementById("custom-ua").checked;
    const userAgent = document.getElementById("user-agent").value;

    const enabledMethods = [];
    if (document.getElementById("push-method-bark").checked) {
      enabledMethods.push("bark");
    }
    if (document.getElementById("push-method-pushplus").checked) {
      enabledMethods.push("pushplus");
    }
    if (document.getElementById("push-method-fangtang").checked) {
      enabledMethods.push("fangtang");
    }
    if (document.getElementById("push-method-dingtalk").checked) {
      enabledMethods.push("dingtalk");
    }
    if (document.getElementById("push-method-wechat").checked) {
      enabledMethods.push("wechat");
    }
    if (document.getElementById("push-method-smtp").checked) {
      enabledMethods.push("smtp");
    }
    if (document.getElementById("push-method-gotify").checked) {
      enabledMethods.push("gotify");
    }

    if (enablePush && enabledMethods.length === 0) {
      showError("启用推送时，必须至少选择一个推送渠道");
      return;
    }

    if (delayTime < 1 || delayTime > 10) {
      showError("延迟时间必须在1-10秒之间");
      return;
    }

    if (maxAttempts < 1 || maxAttempts > 1000) {
      showError("最大尝试次数必须在1-1000之间");
      return;
    }

    if (gotifyUrl && !gotifyUrl.startsWith("http")) {
      showError("Gotify URL必须以http://或https://开头");
      return;
    }

    if (smtpServer && !smtpPort) {
      showError("请填写SMTP端口");
      return;
    }

    if (customUa && !userAgent.trim()) {
      showError("启用自定义User-Agent时，必须填写User-Agent");
      return;
    }

    await invoke("save_settings", {
      grabMode: parseInt(grabMode),
      delayTime: parseInt(delayTime),
      maxAttempts: parseInt(maxAttempts),
      enablePush: enablePush,
      enabledMethods: enabledMethods,
      barkToken: barkToken,
      pushplusToken: pushplusToken,
      fangtangToken: fangtangToken,
      dingtalkToken: dingtalkToken,
      wechatToken: wechatToken,
      gotifyUrl: gotifyUrl,
      gotifyToken: gotifyToken,
      smtpServer: smtpServer,
      smtpPort: smtpPort,
      smtpUsername: smtpUsername,
      smtpPassword: smtpPassword,
      smtpFrom: smtpFrom,
      smtpTo: smtpTo,
      customUa: customUa,
      userAgent: userAgent,
    });

    showSuccess("设置保存成功");
    await loadSettings();
  } catch (error) {
    showError("设置保存失败: " + error);
  }
}

function resetSettings() {
  if (confirm("确定要恢复默认设置吗？")) {
    document.getElementById("grab-mode").value = "0";
    document.getElementById("delay-time").value = "2";
    document.getElementById("max-attempts").value = "100";
    document.getElementById("enable-push").checked = false;
    document.getElementById("bark-token").value = "";
    document.getElementById("pushplus-token").value = "";
    document.getElementById("fangtang-token").value = "";
    document.getElementById("dingtalk-token").value = "";
    document.getElementById("wechat-token").value = "";
    document.getElementById("gotify-url").value = "";
    document.getElementById("gotify-token").value = "";
    document.getElementById("smtp-server").value = "";
    document.getElementById("smtp-port").value = "";
    document.getElementById("smtp-username").value = "";
    document.getElementById("smtp-password").value = "";
    document.getElementById("smtp-from").value = "";
    document.getElementById("smtp-to").value = "";
    document.getElementById("custom-ua").checked = false;
    document.getElementById("user-agent").value = "";
    showSuccess("设置已恢复默认");
  }
}

function updateUptime() {
  const startTime = Date.now();
  setInterval(() => {
    const elapsed = Date.now() - startTime;
    const minutes = Math.floor(elapsed / 60000);
    const hours = Math.floor(minutes / 60);
    const remainingMinutes = minutes % 60;

    let uptimeText = "";
    if (hours > 0) {
      uptimeText = `${hours} 小时 ${remainingMinutes} 分钟`;
    } else {
      uptimeText = `${minutes} 分钟`;
    }

    document.getElementById("uptime").textContent = uptimeText;
  }, 60000);
}

function updateMonitorStats() {
  document.getElementById("monitor-attempts").textContent =
    monitorStats.attempts;
  document.getElementById("monitor-success").textContent = monitorStats.success;
  document.getElementById("monitor-failures").textContent =
    monitorStats.failures;
}

function resetMonitorStats() {
  monitorStats = {
    attempts: 0,
    success: 0,
    failures: 0,
  };
  updateMonitorStats();
}

// Initialization
async function init() {
  console.log("Starting application initialization...");

  try {
    updateUptime();
    await updateSystemInfo();

    await loadAccounts();
    await loadSettings();
    setupPushSettingsEventListeners();

    resetMonitorStats();

    // 初始化抢票日志
    await initGrabLogs();

    setInterval(() => {
      const logsTab = document.getElementById("tab-logs");
      if (logsTab && logsTab.classList.contains("active")) {
        loadLogs();
      }
    }, 5000);

    setInterval(() => {
      const monitorTab = document.getElementById("tab-monitor");
      if (monitorTab && monitorTab.classList.contains("active")) {
        refreshMonitor();
      }
    }, 3000);

    setInterval(async () => {
      const accountsTab = document.getElementById("tab-accounts");
      if (accountsTab && accountsTab.classList.contains("active")) {
        await reloadAccounts();
      }
    }, 30000);

    console.log("Application initialization complete");
  } catch (error) {
    console.error("Initialization error:", error);
  }
}

async function updateSystemInfo() {
  try {
    if (!invoke) {
      return;
    }

    const appInfo = await invoke("get_app_info");
    if (appInfo) {
      const versionElement = document.querySelector(".app-version");
      if (versionElement) {
        versionElement.textContent = `v${appInfo.version}`;
      }

      if (appInfo.machine_id) {
        console.log("Machine ID:", appInfo.machine_id);
      }
    }
  } catch (error) {
    console.error("更新系统信息失败:", error);
  }
}

function switchTab(tabName) {
  console.log("switchTab called with:", tabName);

  if (!tabName) {
    console.error("No tab name provided");
    return;
  }

  document.querySelectorAll(".nav-tab").forEach((tab) => {
    tab.classList.remove("active");
  });

  document.querySelectorAll(".tab-content").forEach((content) => {
    content.classList.remove("active");
  });

  const clickedTab = document.querySelector(`[data-tab="${tabName}"]`);
  if (clickedTab) {
    clickedTab.classList.add("active");
  }

  const targetContent = document.getElementById(`tab-${tabName}`);
  if (targetContent) {
    targetContent.classList.add("active");
    console.log(`Successfully switched to tab: ${tabName}`);

    if (tabName === "projects") {
      if (typeof loadProjects === "function") {
        loadProjects();
      }
    } else if (tabName === "monitor") {
      if (typeof refreshMonitor === "function") {
        refreshMonitor();
      }
      if (typeof resetMonitorStats === "function") {
        resetMonitorStats();
      }
    } else if (tabName === "logs") {
      if (typeof loadLogs === "function") {
        loadLogs();
      }
    } else if (tabName === "settings") {
      if (typeof loadSettings === "function") {
        loadSettings();
      }
    } else if (tabName === "accounts") {
      if (typeof reloadAccounts === "function") {
        reloadAccounts();
      }
    }
  } else {
    console.error(`Tab content not found: tab-${tabName}`);
  }
}

async function testPush() {
  if (!invoke) {
    showError("Tauri API不可用，无法测试推送");
    return;
  }

  if (!confirm("确定要发送测试推送吗？")) {
    return;
  }

  try {
    const result = await invoke("push_test", {
      title: "测试",
      message: "BTR 测试推送",
    });
    showSuccess("测试推送已发送：" + result);
  } catch (error) {
    showError("发送失败: " + error);
  }
}

document.addEventListener("keydown", function (e) {
  if (e.ctrlKey && e.key >= "1" && e.key <= "7") {
    e.preventDefault();
    const tabIndex = parseInt(e.key) - 1;
    const tabs = document.querySelectorAll(".nav-tab");
    if (tabIndex < tabs.length) {
      const tabName = tabs[tabIndex].dataset.tab;
      switchTab(tabName);
    }
  }

  if (e.key === "Escape") {
    closeAddAccountModal();
    closeQrcodeModal();
    closeAddProjectModal();
  }
});

let grabLogs = [];
let autoScrollEnabled = true;
let logFilters = {
  info: true,
  debug: true,
  warn: true,
  error: true,
  success: true,
};

async function loadGrabLogs() {
  const container = document.getElementById("grab-logs-container");
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }
    const logs = await invoke("get_grab_logs");

    if (logs && logs.length > 0) {
      grabLogs = logs;
      updateGrabLogsDisplay();
    } else {
      container.innerHTML = '<div class="log-entry">暂无抢票日志</div>';
      updateLogStats();
    }
  } catch (error) {
    console.error("Failed to load grab logs:", error);
    container.innerHTML = `<div style="color: var(--error-color);">加载抢票日志失败: ${error.message}</div>`;
    updateLogStats();
  }
}

function updateGrabLogsDisplay() {
  const container = document.getElementById("grab-logs-container");
  const filteredLogs = grabLogs.filter((log) => {
    if (log.includes("INFO:")) return logFilters.info;
    if (log.includes("DEBUG:")) return logFilters.debug;
    if (log.includes("WARN:")) return logFilters.warn;
    if (log.includes("ERROR:")) return logFilters.error;
    return logFilters.success;
  });

  if (filteredLogs.length > 0) {
    container.innerHTML = filteredLogs
      .map((log) => formatLogEntry(log))
      .join("");

    if (autoScrollEnabled) {
      container.scrollTop = container.scrollHeight;
    }
  } else {
    container.innerHTML = '<div class="log-entry">暂无符合条件的日志</div>';
  }

  updateLogStats();
}

function formatLogEntry(log) {
  let levelClass = "";
  let levelText = "";

  if (log.includes("INFO:")) {
    levelClass = "info";
    levelText = "INFO";
  } else if (log.includes("DEBUG:")) {
    levelClass = "debug";
    levelText = "DEBUG";
  } else if (log.includes("WARN:")) {
    levelClass = "warn";
    levelText = "WARN";
  } else if (log.includes("ERROR:")) {
    levelClass = "error";
    levelText = "ERROR";
  } else {
    levelClass = "success";
    levelText = "SUCCESS";
  }

  const messageMatch = log.match(
    /\[.*?\]\s*(?:INFO|DEBUG|WARN|ERROR|SUCCESS)?:?\s*(.*)/,
  );
  const message = messageMatch ? messageMatch[1] : log;

  return `
    <div class="log-entry ${levelClass}">
      <span class="log-level ${levelClass}">${levelText}</span>
      <span class="log-message">${message}</span>
    </div>
  `;
}

function updateLogStats() {
  const infoCount = grabLogs.filter((log) => log.includes("INFO:")).length;
  const debugCount = grabLogs.filter((log) => log.includes("DEBUG:")).length;
  const warnCount = grabLogs.filter((log) => log.includes("WARN:")).length;
  const errorCount = grabLogs.filter((log) => log.includes("ERROR:")).length;
  const successCount = grabLogs.filter(
    (log) =>
      !log.includes("INFO:") &&
      !log.includes("DEBUG:") &&
      !log.includes("WARN:") &&
      !log.includes("ERROR:"),
  ).length;

  document.getElementById("grab-log-count").textContent = grabLogs.length;
  document.getElementById("info-count").textContent = infoCount;
  document.getElementById("debug-count").textContent = debugCount;
  document.getElementById("warn-count").textContent = warnCount;
  document.getElementById("error-count").textContent = errorCount;
}

async function clearGrabLogs() {
  if (!confirm("确定要清空所有抢票日志吗？此操作不可撤销！")) return;

  try {
    await invoke("clear_grab_logs");
    grabLogs = [];
    updateGrabLogsDisplay();
    showSuccess("抢票日志已清空");
  } catch (error) {
    showError("清空抢票日志失败: " + error);
  }
}

async function exportGrabLogs() {
  try {
    const logs = grabLogs.join("\n");
    const blob = new Blob([logs], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `grab_logs_${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.txt`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    showSuccess("抢票日志导出成功");
  } catch (error) {
    showError("导出抢票日志失败: " + error);
  }
}

function updatePushSettingsVisibility() {
  const methods = [
    "bark",
    "pushplus",
    "fangtang",
    "dingtalk",
    "wechat",
    "smtp",
    "gotify",
  ];
  methods.forEach((method) => {
    const checkbox = document.getElementById(`push-method-${method}`);
    const settings = document.getElementById(`${method}-settings`);
    if (checkbox && settings) {
      settings.style.display = checkbox.checked ? "block" : "none";
    }
  });
}

function setupPushSettingsEventListeners() {
  const methods = [
    "bark",
    "pushplus",
    "fangtang",
    "dingtalk",
    "wechat",
    "smtp",
    "gotify",
  ];
  methods.forEach((method) => {
    const checkbox = document.getElementById(`push-method-${method}`);
    if (checkbox) {
      checkbox.addEventListener("change", updatePushSettingsVisibility);
    }
  });
}

function toggleAutoScroll() {
  autoScrollEnabled = !autoScrollEnabled;
  const button = document.getElementById("auto-scroll-btn");
  button.textContent = `自动滚动: ${autoScrollEnabled ? "开启" : "关闭"}`;
  button.classList.toggle("btn-info", autoScrollEnabled);
  button.classList.toggle("btn-secondary", !autoScrollEnabled);
}

function toggleLogFilter(level) {
  logFilters[level] = !logFilters[level];
  const button = document.getElementById(`filter-${level}-btn`);
  if (button) {
    button.classList.toggle("active", logFilters[level]);
  }
  updateGrabLogsDisplay();
}

function setupGrabLogsEventListeners() {
  // 按钮事件监听
  document
    .getElementById("refresh-grab-logs-btn")
    ?.addEventListener("click", loadGrabLogs);
  document
    .getElementById("clear-grab-logs-btn")
    ?.addEventListener("click", clearGrabLogs);
  document
    .getElementById("export-grab-logs-btn")
    ?.addEventListener("click", exportGrabLogs);
  document
    .getElementById("auto-scroll-btn")
    ?.addEventListener("click", toggleAutoScroll);

  // 过滤器按钮事件监听
  document
    .getElementById("filter-info-btn")
    ?.addEventListener("click", () => toggleLogFilter("info"));
  document
    .getElementById("filter-debug-btn")
    ?.addEventListener("click", () => toggleLogFilter("debug"));
  document
    .getElementById("filter-warn-btn")
    ?.addEventListener("click", () => toggleLogFilter("warn"));
  document
    .getElementById("filter-error-btn")
    ?.addEventListener("click", () => toggleLogFilter("error"));
  document
    .getElementById("filter-success-btn")
    ?.addEventListener("click", () => toggleLogFilter("success"));

  // 搜索功能
  const searchInput = document.getElementById("log-search");
  if (searchInput) {
    searchInput.addEventListener("input", function () {
      const searchTerm = this.value.toLowerCase();
      const container = document.getElementById("grab-logs-container");
      const logEntries = container.querySelectorAll(".log-entry");

      logEntries.forEach((entry) => {
        const text = entry.textContent.toLowerCase();
        entry.style.display = text.includes(searchTerm) ? "" : "none";
      });
    });
  }
}

// 初始化抢票日志
async function initGrabLogs() {
  setupGrabLogsEventListeners();
  await loadGrabLogs();

  // 设置自动刷新
  setInterval(async () => {
    const grabLogsTab = document.getElementById("tab-grab-logs");
    if (grabLogsTab.classList.contains("active")) {
      await loadGrabLogs();
    }
  }, 3000); // 每3秒刷新一次
}

window.addEventListener("beforeunload", function () {
  if (qrcodePollingInterval) {
    clearInterval(qrcodePollingInterval);
  }
});
