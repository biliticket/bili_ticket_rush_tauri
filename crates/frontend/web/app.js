let invoke = null;

let smsCaptchaKey = null;
let currentTaskId = null;
let isGrabTaskRunning = false; 

async function initializeEventSystem() {
  if (window.__TAURI__ && window.__TAURI__.event) {
    await window.__TAURI__.event.listen('task-update', (event) => {
      handleTaskUpdate(event.payload);
    });
    console.log("Event system initialized");
  } else {
    console.error("Tauri event system not available");
  }
}

function handleTaskUpdate(result) {
  if (!result) return;
  
  const type = Object.keys(result)[0];
  const data = result[type];

  switch (type) {
    case "QrCodeLoginResult":
      handleQrCodeResult(data);
      break;
    case "PasswordLoginResult":
      handlePasswordLoginResult(data);
      break;
    case "SubmitSmsLoginResult":
      handleSubmitSmsLoginResult(data);
      break;
    case "LoginSmsResult":
      handleLoginSmsResult(data);
      break;
    case "GetTicketInfoResult":
      handleGetTicketInfoResult(data);
      break;
    case "GetBuyerInfoResult":
      handleGetBuyerInfoResult(data);
      break;
    case "GrabTicketResult":
      handleGrabTicketResult(data);
      break;
    default:
      console.log("Unknown task result type:", type);
  }
}

function handleQrCodeResult(data) {
    const statusText = document.getElementById("qrcode-status-text");
    const overlay = document.getElementById("qrcode-expired-overlay");

    let status = data.status;
    let statusType = typeof status === 'string' ? status : Object.keys(status)[0];
    let statusValue = typeof status === 'string' ? null : status[statusType];

    if (statusType === "Expired") {
        if (overlay) overlay.style.display = "flex";
        if (statusText) statusText.textContent = "二维码已过期，请刷新";
        showWarning("二维码已过期，请刷新二维码");
    } else if (statusType === "Success") {
        let cookie = statusValue;
        if (cookie) {
            closeAddAccountModal();
            invoke("add_account_by_cookie", { cookie: cookie }).then(() => {
                showSuccess("登录成功！账号已添加");
                reloadAccounts();
            }).catch(err => {
                showError("登录成功但添加账号失败: " + err);
            });
        }
    } else if (statusType === "Scanning") {
        if (statusText) statusText.textContent = "二维码已扫描，等待确认";
        if (overlay) overlay.style.display = "none";
    } else if (statusType === "Confirming") {
        if (statusText) statusText.textContent = "二维码已确认，正在登录";
        if (overlay) overlay.style.display = "none";
    } else if (statusType === "Failed") {
        if (statusText) statusText.textContent = "登录失败: " + statusValue;
    }
}

function handlePasswordLoginResult(data) {
    if (data.success && data.cookie) {
        closeAddAccountModal();
        invoke("add_account_by_cookie", { cookie: data.cookie }).then(() => {
            showSuccess("密码登录成功！账号已添加");
            reloadAccounts();
        }).catch(err => {
            showError("密码登录成功但添加账号失败: " + err);
        });
    } else {
        showError("密码登录失败: " + data.message);
    }
}

function handleSubmitSmsLoginResult(data) {
    if (data.success && data.cookie) {
        closeAddAccountModal();
        invoke("add_account_by_cookie", { cookie: data.cookie }).then(() => {
            showSuccess("手机号登录成功！账号已添加");
            reloadAccounts();
        }).catch(err => {
            showError("手机号登录成功但添加账号失败: " + err);
        });
    } else {
        showError("手机号登录失败: " + data.message);
    }
}

function handleLoginSmsResult(data) {
    if (data.success) {
        showSuccess("短信验证码已发送！");
        smsCaptchaKey = data.message;
    } else {
        showError("发送短信验证码失败: " + data.message);
    }
}

function handleGetTicketInfoResult(data) {
    const modal = document.getElementById("screen-ticket-modal");
    if (!modal.classList.contains("active")) return;
    
    if (data.success && data.ticket_info) {
        showScreenTicketSelector(data.ticket_info.data);
    } else {
        showError(data.message || "获取项目详情失败");
        closeScreenTicketModal();
    }
}

function handleGetBuyerInfoResult(data) {
    const buyerLoading = document.getElementById("buyer-loading");
    if (buyerLoading.style.display === "none") return; 

    if (data.success && data.buyer_info) {
        displayBuyerList(data.buyer_info);
        document.getElementById("buyer-loading").style.display = "none";
        document.getElementById("buyer-list").style.display = "block";
    } else {
        document.getElementById("buyer-loading").style.display = "none";
        document.getElementById("buyer-error").style.display = "block";
        document.getElementById("buyer-error").textContent = "加载购票人失败: " + data.message;
    }
}

function handleGrabTicketResult(data) {
    if (data.task_id !== currentTaskId) return;

    if (data.success) {
        try {
            showGrabSuccessModal(data);
        } catch (e) {
            console.error("显示成功弹窗失败:", e);
            showError("抢票成功，但显示详情失败: " + e.message);
        } finally {
            stopGrab();
        }
    } else if (data.message && data.message.includes("待付款订单")) {
        showError(data.message);
        stopGrab();
    }
}

function showAddAccountModal() {
  const modal = document.getElementById("add-account-modal");
  if (modal) {
    modal.classList.add("active");
    switchLoginMethod("qrcode");
  } else {
    console.error("Add account modal not found");
  }
}

function switchLoginMethod(method) {
  document.querySelectorAll(".login-tab").forEach(tab => {
    if (tab.dataset.method === method) {
      tab.classList.add("active");
    } else {
      tab.classList.remove("active");
    }
  });

  document.querySelectorAll(".login-method-content").forEach(content => {
    if (content.id === `method-${method}`) {
      content.classList.add("active");
    } else {
      content.classList.remove("active");
    }
  });

  if (method === "qrcode") {
    refreshQrcode();
  }
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

function closeAddProjectModal() {
  const modal = document.getElementById("add-project-modal");
  if (modal) {
    modal.classList.remove("active");
  }

  document.getElementById("project-id").value = "";
  document.getElementById("project-name").value = "";
}

function showGrabSuccessModal(result) {
    const modal = document.getElementById('grab-success-modal');
    if (!modal) return;

    try {
        const confirmResult = result.confirm_result;
        const payResult = result.pay_result;

        if (confirmResult) {
            const projectName = document.getElementById('success-project-name');
            if (projectName) projectName.textContent = confirmResult.project_name || 'N/A';
            
            const screenName = document.getElementById('success-screen-name');
            if (screenName) screenName.textContent = confirmResult.screen_name || 'N/A';
            
            if (confirmResult.ticket_info) {
                const ticketName = document.getElementById('success-ticket-name');
                if (ticketName) ticketName.textContent = confirmResult.ticket_info.name || 'N/A';
                
                const ticketPrice = document.getElementById('success-ticket-price');
                if (ticketPrice) {
                    const price = confirmResult.ticket_info.price || 0;
                    const count = confirmResult.count || 1;
                    ticketPrice.textContent = ((price * count) / 100).toFixed(2);
                }
            }
        }

        const qrImg = document.getElementById('payment-qrcode-img');
        if (qrImg) {
            if (payResult && payResult.code_url) {
                const qrCodeApiUrl = `https://api.2dcode.biz/v1/create-qr-code?data=${encodeURIComponent(payResult.code_url)}&size=200x200`;
                qrImg.src = qrCodeApiUrl;
                qrImg.style.display = 'block';
            } else {
                qrImg.src = '';
                qrImg.style.display = 'none';
            }
        }

        modal.classList.add('active');
    } catch (e) {
        throw e;
    }
}

function closeGrabSuccessModal() {
    const modal = document.getElementById('grab-success-modal');
    if (modal) {
        modal.classList.remove('active');
    }
}


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


async function requestSmsCode() {
  const phoneNumber = document.getElementById("phone-login-phone").value.trim();
  const cid = parseInt(document.getElementById("phone-login-cid").value) || 86;
  if (!phoneNumber) {
    showWarning("请输入手机号");
    return;
  }

  const sendSmsButton = document.getElementById("phone-login-send-sms-btn");
  sendSmsButton.disabled = true;
  const originalButtonText = sendSmsButton.textContent;
  let countdown = 60;
  sendSmsButton.textContent = `重新发送 (${countdown}s)`;

  const countdownInterval = setInterval(() => {
    countdown--;
    if (countdown > 0) {
      sendSmsButton.textContent = `重新发送 (${countdown}s)`;
    } else {
      clearInterval(countdownInterval);
      sendSmsButton.textContent = originalButtonText;
      sendSmsButton.disabled = false;
    }
  }, 1000);

  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }
    const taskId = await invoke("send_loginsms_command", { phoneNumber, cid });
    showSuccess("发送短信请求已提交...");
  } catch (error) {
    showError("请求短信验证码失败: " + error);
    clearInterval(countdownInterval);
    sendSmsButton.textContent = originalButtonText;
    sendSmsButton.disabled = false;
  }
}

async function submitPhoneLogin() {
  const phoneNumber = document.getElementById("phone-login-phone").value.trim();
  const smsCode = document.getElementById("phone-login-sms-code").value.trim();
  const cid = parseInt(document.getElementById("phone-login-cid").value) || 86;

  if (!phoneNumber || !smsCode) {
    showWarning("请输入手机号和验证码");
    return;
  }
  if (!smsCaptchaKey) {
    showWarning("请先获取短信验证码");
    return;
  }

  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }
    const taskId = await invoke("submit_loginsms_command", { phoneNumber, cid, smsCode, captchaKey: smsCaptchaKey });
    showSuccess("手机号登录任务已提交...");
  } catch (error) {
    showError("手机号登录失败: " + error);
  }
}

async function submitPasswordLogin() {
  const username = document.getElementById("password-login-username").value.trim();
  const password = document.getElementById("password-login-password").value.trim();

  if (!username || !password) {
    showWarning("请输入用户名和密码");
    return;
  }

  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }
    const taskId = await invoke("password_login_command", { username, password });
    showSuccess("密码登录任务已提交...");
  } catch (error) {
    showError("密码登录失败: " + error);
  }
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

  document.querySelectorAll(".login-tab").forEach((tab) => {
    const method = tab.getAttribute("data-method");
    if (method) {
      tab.addEventListener("click", () => {
        switchLoginMethod(method);
      });
    }
  });

  const buttonIds = {
    "add-account-btn": showAddAccountModal,
    "reload-accounts-btn": reloadAccounts,
    "phone-login-send-sms-btn": requestSmsCode,
    "phone-login-submit-btn": submitPhoneLogin,
    "password-login-submit-btn": submitPasswordLogin,
    "start-grab-btn": startGrab,
    "stop-grab-btn": stopGrab,
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

  document.getElementById("grab-mode")?.addEventListener("change", updateSkipWordsVisibility);
  document.getElementById("enable-push")?.addEventListener("change", updatePushSettingsVisibility);
}

document.addEventListener("DOMContentLoaded", function () {
  console.log("DOM loaded, initializing application...");

  initializeTabSwitching();
  initializeEventListeners();

  document.getElementById("start-grab-btn").disabled = false;
  document.getElementById("stop-grab-btn").disabled = true;

  let attempts = 0;
  const maxAttempts = 20;
  const checkInterval = 100;

  function checkTauriAvailability() {
    attempts++;
    console.log(`Checking for Tauri API (attempt ${attempts}/${maxAttempts})...`);

    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      console.log("Tauri API found!");
      initializeApp();
    } else if (attempts < maxAttempts) {
      setTimeout(checkTauriAvailability, checkInterval);
    } else {
      console.log("Tauri API not available after " + maxAttempts * checkInterval + "ms, using basic UI");
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

    initializeEventSystem();

    if (window.__TAURI__.event && window.__TAURI__.event.listen) {
      window.__TAURI__.event.listen('log-event', (event) => {
        handleIncomingLog(event.payload);
      });
    }

    init();
  } catch (error) {
    console.error("Failed to initialize Tauri API:", error);
    initializeBasicUI();
  }
}

function handleIncomingLog(log) {
  if (!allLogs.includes(log)) {
    allLogs.push(log);
    if (allLogs.length > 5000) {
      allLogs.shift();
    }

    const logCount = document.getElementById("log-count");
    if (logCount) {
      logCount.textContent = allLogs.length;
    }

    const grabTab = document.getElementById("tab-grab");
    if (grabTab && grabTab.classList.contains("active")) {
        appendLogEntry(log);
    } else {
      updateLogStats();
    }
  }
}

function appendLogEntry(log) {
    const container = document.getElementById("grab-logs-container");
    if (!container) return;

    let visible = false;
    if (log.includes("INFO:") && logFilters.info) visible = true;
    else if (log.includes("DEBUG:") && logFilters.debug) visible = true;
    else if (log.includes("WARN:") && logFilters.warn) visible = true;
    else if (log.includes("ERROR:") && logFilters.error) visible = true;
    else if (logFilters.success && !log.match(/INFO:|DEBUG:|WARN:|ERROR:/)) visible = true;
    const searchTerm = document.getElementById("log-search")?.value.toLowerCase();
    if (visible && searchTerm && !log.toLowerCase().includes(searchTerm)) {
        visible = false;
    }

    if (visible) {
        if (container.firstElementChild && container.firstElementChild.textContent.includes("暂无")) {
            container.innerHTML = "";
        }
        
        container.insertAdjacentHTML('beforeend', formatLogEntry(log));
        
        if (container.childElementCount > 5000) {
            container.firstElementChild.remove();
        }

        if (autoScrollEnabled) {
            container.scrollTop = container.scrollHeight;
        }
    }
    
    updateLogStats();
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

    loadInitialLogs = function () {
      console.log("loadInitialLogs called (mock)");
      const container = document.getElementById("grab-logs-container");
      if (container) {
        container.innerHTML = '<div class="log-entry">Tauri API not available - cannot load logs</div>';
      }
    };

    loadInitialLogs();
  }, 100);
}

function initializeTabSwitching() {
  const navTabs = document.querySelectorAll(".nav-tab");
  navTabs.forEach((tab) => {
    tab.style.cursor = "pointer";
    tab.onclick = function (e) {
      e.preventDefault();
      const tabName = this.dataset.tab;
      switchTab(tabName);
      return false;
    };
  });

  const homeTab = document.querySelector('[data-tab="home"]');
  if (homeTab && !homeTab.classList.contains("active")) {
    homeTab.classList.add("active");
  }
}

async function refreshQrcode() {
  try {
    if (!invoke) {
      throw new Error("Tauri invoke function not available");
    }

    const overlay = document.getElementById("qrcode-expired-overlay");
    if (overlay) overlay.style.display = "none";
    
    const statusText = document.getElementById("qrcode-status-text");
    if (statusText) statusText.textContent = "正在生成二维码...";

    const qrcodeData = await invoke("qrcode_login");

    if (qrcodeData && qrcodeData.url) {
      document.getElementById("qrcode-img").src = qrcodeData.url;
      if (statusText) statusText.textContent = "请使用B站APP扫描二维码登录";
    } else {
      throw new Error("无法生成二维码");
    }
  } catch (error) {
    showError("生成二维码失败: " + error.message);
  }
}

function handleQrcodeClick() {
  const overlay = document.getElementById("qrcode-expired-overlay");
  if (overlay && overlay.style.display !== "none") {
    refreshQrcode();
  }
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
    if (!invoke) throw new Error("Tauri invoke function not available");
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

  if (!projectId || !projectName) {
    showWarning("请填写所有字段");
    return;
  }

  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    if (!/^\d+$/.test(projectId)) {
      showWarning("项目ID必须为数字");
      return;
    }

    await invoke("add_project", { id: projectId, name: projectName });
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
    if (!invoke) throw new Error("Tauri invoke function not available");
    const accounts = await invoke("get_accounts");

    loading.style.display = "none";
    list.style.display = "block";
    list.innerHTML = "";

    if (accounts.length === 0) {
      list.innerHTML = '<li style="padding: 20px; text-align: center; color: var(--text-secondary);">暂无账号</li>';
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

    document.getElementById("account-count").textContent = accounts.length;
  } catch (error) {
    console.error("Failed to load accounts:", error);
    loading.style.display = "none";
    list.style.display = "block";
    list.innerHTML = `<div style="padding: 20px; text-align: center; color: var(--error-color);">加载失败: ${error.message}</div>`;
  }
}

async function toggleAccountActive(uid, active) {
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    await invoke("set_account_active", { uid, active });
  } catch (error) {
    showError("更新账号状态失败: " + error);
    await reloadAccounts();
  }
}

async function reloadAccounts() {
  const loading = document.getElementById("accounts-loading");
  if (loading) loading.style.display = "block";
  const list = document.getElementById("accounts-list");
  if (list) list.style.display = "none";
  await loadAccounts();
}

async function deleteAccount(uid) {
  if (!confirm("确定要删除此账号吗？")) return;
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    await invoke("delete_account_by_uid", { uid });
    await loadAccounts();
  } catch (error) {
    showError("删除失败: " + error);
  }
}

async function loadProjects() {
  const loading = document.getElementById("projects-loading");
  const list = document.getElementById("projects-list");

  if (loading) loading.style.display = "block";
  if (list) list.style.display = "none";

  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    const projects = await invoke("get_projects");

    if (list) {
        list.innerHTML = "";
        if (!projects || projects.length === 0) {
          list.innerHTML = '<div style="padding: 40px; text-align: center; color: var(--text-secondary); grid-column: 1 / -1;">暂无项目</div>';
        } else {
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
        }
    }
  } catch (error) {
    console.error("加载项目失败:", error);
    if (list) {
        const errorMsg = typeof error === 'string' ? error : (error.message || JSON.stringify(error));
        list.innerHTML = `<div style="padding: 40px; text-align: center; color: var(--error-color); grid-column: 1 / -1;">加载失败: ${errorMsg}</div>`;
    }
  } finally {
    if (loading) loading.style.display = "none";
    if (list) list.style.display = "grid";
  }
}

function addProject() {
  showAddProjectModal();
}

async function refreshProjects() {
  await loadProjects();
}

async function selectProject(projectId) {
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    await invoke("set_ticket_id", { ticketId: projectId });
    const accounts = await invoke("get_accounts");
    const activeAccount = accounts.find((a) => a.is_active);
    if (!activeAccount) {
      showWarning("请先激活一个账号");
      return;
    }
    showScreenTicketModal();
    await invoke("get_ticket_info", { uid: activeAccount.uid, projectId: projectId });
  } catch (error) {
    showError("选择项目失败: " + error);
    closeScreenTicketModal();
  }
}

function showScreenTicketModal() {
  const modal = document.getElementById("screen-ticket-modal");
  const loading = document.getElementById("screen-ticket-loading");
  const selector = document.getElementById("screen-ticket-selector");
  modal.classList.add("active");
  loading.style.display = "block";
  selector.style.display = "none";
}

function closeScreenTicketModal() {
  const modal = document.getElementById("screen-ticket-modal");
  modal.classList.remove("active");
  document.getElementById("screen-select").innerHTML = "";
  document.getElementById("ticket-select").innerHTML = "";
  document.getElementById("buyer-list").innerHTML = "";
  document.getElementById("no-bind-name").value = "";
  document.getElementById("no-bind-tel").value = "";
}

async function showScreenTicketSelector(ticketInfo) {
  const loading = document.getElementById("screen-ticket-loading");
  const selector = document.getElementById("screen-ticket-selector");
  const screenSelect = document.getElementById("screen-select");

  loading.style.display = "none";
  selector.style.display = "block";

  const availableScreens = ticketInfo.screen_list.filter((s) => s.clickable !== false);
  if (availableScreens.length === 0) {
    showWarning("暂无可选场次");
    closeScreenTicketModal();
    return;
  }

  screenSelect.innerHTML = availableScreens.map(s => `<option value="${s.id}">${s.name} (${new Date(s.start_time * 1000).toLocaleString()})</option>`).join("");
  window.currentTicketInfo = ticketInfo;
  screenSelect.onchange = function () { updateTicketList(parseInt(this.value)); };
  updateTicketList(availableScreens[0].id);

  const idBind = ticketInfo.id_bind;
  const realNameSection = document.getElementById("real-name-buyer-section");
  const nonRealNameSection = document.getElementById("non-real-name-buyer-section");

  if (idBind === 0) {
    realNameSection.style.display = "none";
    nonRealNameSection.style.display = "block";
  } else {
    realNameSection.style.display = "block";
    nonRealNameSection.style.display = "none";
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
  ticketSelect.innerHTML = selectedScreen.ticket_list.map((t) => {
      const price = (t.price / 100).toFixed(2);
      const status = t.sale_type === 1 ? "可售" : t.sale_type === 2 ? "售罄" : "未开售";
      return `<option value="${t.id}">${t.desc} - ¥${price} [${status}]</option>`;
  }).join("");
}

async function saveNoBindBuyerInfo() {
  const name = document.getElementById("no-bind-name").value.trim();
  const tel = document.getElementById("no-bind-tel").value.trim();

  if (!name || !tel) {
    showWarning("请填写姓名和手机号");
    return;
  }

  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    await invoke("set_no_bind_buyer_info", { name, tel });
    showSuccess("非实名购票人信息保存成功");
  } catch (error) {
    showError("保存失败: " + error);
  }
}

async function confirmScreenTicketSelection() {
  try {
    const screenId = parseInt(document.getElementById("screen-select").value);
    const ticketId = parseInt(document.getElementById("ticket-select").value);
    if (!screenId || !ticketId) {
      showWarning("请选择场次和票种");
      return;
    }
    const realNameSection = document.getElementById("real-name-buyer-section");
    const nonRealNameSection = document.getElementById("non-real-name-buyer-section");
    let buyerType = realNameSection.style.display !== "none" ? "1" : "0";

    if (buyerType === "1") {
      const selectedBuyers = getSelectedBuyers();
      if (selectedBuyers.length === 0) {
        showWarning("请至少选择一个购票人");
        return;
      }
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
      await invoke("set_selected_buyer_list", { buyerList: validatedBuyers });
      await invoke("clear_no_bind_buyer_info");
    } else {
      const name = document.getElementById("no-bind-name").value.trim();
      const tel = document.getElementById("no-bind-tel").value.trim();
      if (!name || !tel) {
        showWarning("请填写姓名和手机号");
        return;
      }
      await invoke("set_no_bind_buyer_info", { name, tel });
      await invoke("set_selected_buyer_list", { buyerList: null });
    }

    await invoke("set_selected_screen", { id: screenId });
    await invoke("set_selected_ticket", { id: ticketId });
    await invoke("set_buyer_type", { buyerType: parseInt(buyerType) });

    showSuccess("设置成功");
    closeScreenTicketModal();
  } catch (error) {
    showError("设置失败: " + error);
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
    if (!activeAccount) throw new Error("请先激活一个账号");
    await invoke("get_buyer_info", { uid: activeAccount.uid });
  } catch (error) {
    buyerLoading.style.display = "none";
    buyerError.style.display = "block";
    buyerError.textContent = "加载购票人失败: " + error.message;
  }
}

function displayBuyerList(buyerInfo) {
  const buyerList = document.getElementById("buyer-list");
  if (!buyerInfo || !buyerInfo.data || !buyerInfo.data.list || buyerInfo.data.list.length === 0) {
    buyerList.innerHTML = '<p style="color: #888; padding: 10px;">暂无购票人</p>';
    return;
  }
  buyerList.innerHTML = buyerInfo.data.list.map(buyer => `
        <div class="buyer-item" style="display: flex; align-items: center; padding: 10px; border-bottom: 1px solid #eee;">
          <input type="checkbox" id="buyer-${buyer.id}" value="${buyer.id}" data-buyer='${encodeURIComponent(JSON.stringify(buyer))}' style="margin-right: 10px;" />
          <label for="buyer-${buyer.id}" style="flex: 1; cursor: pointer;"><strong>${buyer.name}</strong> - ${buyer.tel}</label>
        </div>
      `).join("");
}

function getSelectedBuyers() {
  const checkboxes = document.querySelectorAll('#buyer-list input[type="checkbox"]:checked');
  const selectedBuyers = [];
  checkboxes.forEach((checkbox) => {
    try {
      const buyerData = JSON.parse(decodeURIComponent(checkbox.getAttribute("data-buyer")));
      selectedBuyers.push(buyerData);
    } catch (e) { console.error("解析购票人数据失败:", e); }
  });
  return selectedBuyers;
}

async function deleteProject(projectId) {
  if (!confirm("确定要删除此项目吗？")) return;
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    await invoke("delete_project", { id: projectId });
    showSuccess("删除项目成功");
    await loadProjects();
  } catch (error) { showError("删除失败: " + error); }
}

async function startGrab() {
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    if (isGrabTaskRunning) { showWarning("任务已在运行"); return; }
    isGrabTaskRunning = true;
    document.getElementById("start-grab-btn").disabled = true;
    document.getElementById("stop-grab-btn").disabled = false;
    const state = await invoke("get_state");
    await invoke("set_grab_mode", { mode: state.grab_mode });
    const taskId = await invoke("start_grab_ticket");
    currentTaskId = taskId;
    document.getElementById("monitor-status").textContent = "运行中";
    document.getElementById("monitor-status").style.color = "var(--success-color)";
    showSuccess("开始抢票! 任务ID: " + taskId);
  } catch (error) {
    isGrabTaskRunning = false;
    document.getElementById("start-grab-btn").disabled = false;
    document.getElementById("stop-grab-btn").disabled = true;
    showError("启动失败: " + error);
  }
}

async function stopGrab() {
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    if (currentTaskId) {
      await invoke("cancel_task", { taskId: currentTaskId });
      currentTaskId = null;
    }
    await invoke("set_grab_mode", { mode: 0 });
    document.getElementById("monitor-status").textContent = "已停止";
    document.getElementById("monitor-status").style.color = "var(--error-color)";
    isGrabTaskRunning = false;
    document.getElementById("start-grab-btn").disabled = false;
    document.getElementById("stop-grab-btn").disabled = true;
  } catch (error) { showError("停止失败: " + error); }
}

async function loadSettings() {
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    const state = await invoke("get_state");
    document.getElementById("grab-mode").value = state.grab_mode || "0";
    document.getElementById("delay-time").value = state.status_delay || "2";
    document.getElementById("max-attempts").value = state.config?.max_attempts || "100";
    document.getElementById("skip-words-input").value = state.skip_words ? state.skip_words.join(", ") : "";
    
    // Load retry config
    if (state.custom_config) {
        document.getElementById("max-token-retry").value = state.custom_config.max_token_retry || "5";
        document.getElementById("max-confirm-retry").value = state.custom_config.max_confirm_retry || "4";
        document.getElementById("max-fake-check-retry").value = state.custom_config.max_fake_check_retry || "10";
        document.getElementById("max-order-retry").value = state.custom_config.max_order_retry || "30";
        document.getElementById("retry-interval-ms").value = state.custom_config.retry_interval_ms || "400";
    }

    if (state.push_config) {
      document.getElementById("enable-push").checked = state.push_config.enabled || false;
      document.getElementById("bark-token").value = state.push_config.bark_token || "";
      document.getElementById("pushplus-token").value = state.push_config.pushplus_token || "";
      document.getElementById("fangtang-token").value = state.push_config.fangtang_token || "";
      document.getElementById("dingtalk-token").value = state.push_config.dingtalk_token || "";
      document.getElementById("wechat-token").value = state.push_config.wechat_token || "";
      if (state.push_config.enabled_methods) {
        ["bark", "pushplus", "fangtang", "dingtalk", "wechat", "gotify"].forEach(m => {
          const el = document.getElementById(`push-method-${m}`);
          if (el) el.checked = state.push_config.enabled_methods.includes(m);
        });
      }
      if (state.push_config.gotify_config) {
        document.getElementById("gotify-url").value = state.push_config.gotify_config.gotify_url || "";
        document.getElementById("gotify-token").value = state.push_config.gotify_config.gotify_token || "";
      }
    }
    updatePushSettingsVisibility();
    updateSkipWordsVisibility();
  } catch (error) { console.error("加载设置失败:", error); }
}

function updatePushSettingsVisibility() {
  const pushEnabled = document.getElementById("enable-push").checked;
  const methods = ["bark", "pushplus", "fangtang", "dingtalk", "wechat", "gotify"];
  methods.forEach(m => {
    const el = document.getElementById(`${m}-settings`);
    if (el) el.style.display = pushEnabled ? "block" : "none";
  });
}

function updateSkipWordsVisibility() {
  const mode = document.getElementById("grab-mode").value;
  const container = document.getElementById("skip-words-settings");
  if (container) container.style.display = (mode === "3") ? "block" : "none";
}

async function saveSettings() {
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    const grabMode = parseInt(document.getElementById("grab-mode").value);
    const delayTime = parseInt(document.getElementById("delay-time").value);
    const maxAttempts = parseInt(document.getElementById("max-attempts").value);
    const enablePush = document.getElementById("enable-push").checked;
    const skipWords = document.getElementById("skip-words-input").value.split(",").map(s => s.trim()).filter(s => s.length > 0);
    
    // Read retry config
    const maxTokenRetry = parseInt(document.getElementById("max-token-retry").value) || 5;
    const maxConfirmRetry = parseInt(document.getElementById("max-confirm-retry").value) || 4;
    const maxFakeCheckRetry = parseInt(document.getElementById("max-fake-check-retry").value) || 10;
    const maxOrderRetry = parseInt(document.getElementById("max-order-retry").value) || 30;
    const retryIntervalMs = parseInt(document.getElementById("retry-interval-ms").value) || 400;

    const enabledMethods = ["bark", "pushplus", "fangtang", "dingtalk", "wechat", "gotify"].filter(m => document.getElementById(`push-method-${m}`)?.checked);

    await invoke("save_settings", {
      grabMode, delayTime, maxAttempts, enablePush, enabledMethods,
      barkToken: document.getElementById("bark-token").value,
      pushplusToken: document.getElementById("pushplus-token").value,
      fangtangToken: document.getElementById("fangtang-token").value,
      dingtalkToken: document.getElementById("dingtalk-token").value,
      wechatToken: document.getElementById("wechat-token").value,
      gotifyUrl: document.getElementById("gotify-url").value,
      gotifyToken: document.getElementById("gotify-token").value,
      customUa: false, userAgent: "", skipWords: skipWords.length > 0 ? skipWords : null,
      maxTokenRetry, maxConfirmRetry, maxFakeCheckRetry, maxOrderRetry, retryIntervalMs
    });
    showSuccess("保存成功");
    await loadSettings();
  } catch (error) { showError("保存失败: " + error); }
}

function updateUptime() {
  const startTime = Date.now();
  setInterval(() => {
    const elapsed = Date.now() - startTime;
    const min = Math.floor(elapsed / 60000);
    const h = Math.floor(min / 60);
    document.getElementById("uptime").textContent = h > 0 ? `${h} 小时 ${min % 60} 分钟` : `${min} 分钟`;
  }, 60000);
}

async function updateSystemInfo() {
  try {
    if (!invoke) return;
    const appInfo = await invoke("get_app_info");
    if (appInfo) {
      const el = document.querySelector(".app-version");
      if (el) el.textContent = `v${appInfo.version}`;
    }
  } catch (error) { console.error("系统信息失败:", error); }
}

function switchTab(tabName) {
  document.querySelectorAll(".nav-tab").forEach(t => t.classList.remove("active"));
  document.querySelectorAll(".tab-content").forEach(c => c.classList.remove("active"));
  document.querySelector(`[data-tab="${tabName}"]`)?.classList.add("active");
  const target = document.getElementById(`tab-${tabName}`);
  if (target) {
    target.classList.add("active");
    if (tabName === "grab") loadInitialLogs();
    else if (tabName === "projects") loadProjects();
    else if (tabName === "settings") loadSettings();
    else if (tabName === "accounts") reloadAccounts();
  }
}

async function loadCountryList() {
  const cidSelect = document.getElementById("phone-login-cid");
  if (!cidSelect) return;

  try {
    if (!invoke) return;
    const countries = await invoke("get_country_list_command");
    if (countries && countries.length > 0) {
      cidSelect.innerHTML = countries.map(c => 
        `<option value="${c.cid}" ${c.cid === 86 ? 'selected' : ''}>${c.name} +${c.cid}</option>`
      ).join("");
    }
  } catch (error) {
    console.error("加载地区列表失败:", error);
    cidSelect.innerHTML = '<option value="86">+86</option>';
  }
}

async function init() {
  updateUptime();
  await updateSystemInfo();
  await loadAccounts();
  await loadSettings();
  await initLogs();
  await loadCountryList();
  setInterval(async () => {
    if (document.getElementById("tab-accounts")?.classList.contains("active")) await reloadAccounts();
  }, 30000);
}

let allLogs = [];
let logSet = new Set();
let logStatsCounter = { info: 0, debug: 0, warn: 0, error: 0 };
let autoScrollEnabled = true;
let logFilters = { info: true, debug: true, warn: true, error: true, success: true };

function getLogLevel(log) {
    if (log.includes("INFO:")) return "info";
    if (log.includes("DEBUG:")) return "debug";
    if (log.includes("WARN:")) return "warn";
    if (log.includes("ERROR:")) return "error";
    return "success";
}

function handleIncomingLog(log) {
  if (!logSet.has(log)) {
    logSet.add(log);
    allLogs.push(log);
    
    const level = getLogLevel(log);
    if (logStatsCounter[level] !== undefined) {
        logStatsCounter[level]++;
    }

    if (allLogs.length > 5000) {
      const removed = allLogs.shift();
      logSet.delete(removed);
      const removedLevel = getLogLevel(removed);
      if (logStatsCounter[removedLevel] !== undefined) {
          logStatsCounter[removedLevel]--;
      }
    }

    const logCountEl = document.getElementById("log-count");
    if (logCountEl) {
      logCountEl.textContent = allLogs.length;
    }

    const grabTab = document.getElementById("tab-grab");
    if (grabTab && grabTab.classList.contains("active")) {
        appendLogEntry(log);
    } else {
      updateLogStats();
    }
  }
}

function appendLogEntry(log) {
    const container = document.getElementById("grab-logs-container");
    if (!container) return;

    const level = getLogLevel(log);
    let visible = false;
    if (level === "info" && logFilters.info) visible = true;
    else if (level === "debug" && logFilters.debug) visible = true;
    else if (level === "warn" && logFilters.warn) visible = true;
    else if (level === "error" && logFilters.error) visible = true;
    else if (level === "success" && logFilters.success) visible = true;

    const searchTerm = document.getElementById("log-search")?.value.toLowerCase();
    if (visible && searchTerm && !log.toLowerCase().includes(searchTerm)) {
        visible = false;
    }

    if (visible) {
        if (container.firstElementChild && container.firstElementChild.textContent.includes("暂无")) {
            container.innerHTML = "";
        }
        
        container.insertAdjacentHTML('beforeend', formatLogEntry(log));
        
        if (container.childElementCount > 5000) {
            container.firstElementChild.remove();
        }

        if (autoScrollEnabled) {
            container.scrollTop = container.scrollHeight;
        }
    }
    
    updateLogStats();
}

async function loadInitialLogs() {
  try {
    if (!invoke) throw new Error("Tauri invoke function not available");
    const logs = await invoke("get_logs");
    if (logs && logs.length > 0) { 
        allLogs = logs;
        logSet = new Set(allLogs);
        // 重新初始化统计计数器
        logStatsCounter = { info: 0, debug: 0, warn: 0, error: 0 };
        allLogs.forEach(log => {
            const level = getLogLevel(log);
            if (logStatsCounter[level] !== undefined) logStatsCounter[level]++;
        });
        updateLogsDisplay(); 
    }
    else { 
        document.getElementById("grab-logs-container").innerHTML = '<div class="log-entry">暂无日志</div>'; 
        updateLogStats(); 
    }
  } catch (error) { console.error("加载日志失败:", error); }
}

function updateLogsDisplay() {
  const container = document.getElementById("grab-logs-container");
  if (!container) return;
  const filtered = allLogs.filter(log => {
    const level = getLogLevel(log);
    if (level === "info") return logFilters.info;
    if (level === "debug") return logFilters.debug;
    if (level === "warn") return logFilters.warn;
    if (level === "error") return logFilters.error;
    return logFilters.success;
  });
  if (filtered.length > 0) {
    container.innerHTML = filtered.map(log => formatLogEntry(log)).join("");
    if (autoScrollEnabled) container.scrollTop = container.scrollHeight;
  } else { container.innerHTML = '<div class="log-entry">暂无符合条件的日志</div>'; }
  updateLogStats();
}

function formatLogEntry(log) {
  let level = "success", text = "SUCCESS";
  if (log.includes("INFO:")) { level = "info"; text = "INFO"; }
  else if (log.includes("DEBUG:")) { level = "debug"; text = "DEBUG"; }
  else if (log.includes("WARN:")) { level = "warn"; text = "WARN"; }
  else if (log.includes("ERROR:")) { level = "error"; text = "ERROR"; }
  const match = log.match(/\[.*?\]\s*(?:INFO|DEBUG|WARN|ERROR|SUCCESS)?:?\s*(.*)/);
  const msg = match ? match[1] : log;
  return `<div class="log-entry ${level}"><span class="log-level ${level}">${text}</span><span class="log-message">${msg}</span></div>`;
}

function updateLogStats() {
  document.getElementById("grab-log-count").textContent = allLogs.length;
  document.getElementById("log-count").textContent = allLogs.length;
  ["info", "debug", "warn", "error"].forEach(lv => {
    const el = document.getElementById(`${lv}-count`);
    if (el) el.textContent = logStatsCounter[lv];
  });
}

async function clearAllLogs() {
  if (!confirm("清空日志？")) return;
  try {
    await invoke("clear_logs");
    allLogs = [];
    logSet = new Set();
    logStatsCounter = { info: 0, debug: 0, warn: 0, error: 0 };
    updateLogsDisplay();
  } catch (error) { showError("清空失败: " + error); }
}

function toggleLogFilter(level) {
  logFilters[level] = !logFilters[level];
  document.getElementById(`filter-${level}-btn`)?.classList.toggle("active", logFilters[level]);
  updateLogsDisplay();
}

function toggleAutoScroll() {
  autoScrollEnabled = !autoScrollEnabled;
  const btn = document.getElementById("auto-scroll-btn");
  if (btn) btn.textContent = `自动滚动: ${autoScrollEnabled ? '开启' : '关闭'}`;
}

function setupLogsEventListeners() {
  document.getElementById("refresh-grab-logs-btn")?.addEventListener("click", loadInitialLogs);
  document.getElementById("clear-grab-logs-btn")?.addEventListener("click", clearAllLogs);
  document.getElementById("auto-scroll-btn")?.addEventListener("click", toggleAutoScroll);
  ["info", "debug", "warn", "error", "success"].forEach(lv => {
    document.getElementById(`filter-${lv}-btn`)?.addEventListener("click", () => toggleLogFilter(lv));
  });
  document.getElementById("log-search")?.addEventListener("input", function() {
    const term = this.value.toLowerCase();
    document.querySelectorAll(".log-entry").forEach(el => el.style.display = el.textContent.toLowerCase().includes(term) ? "" : "none");
  });
}

async function initLogs() { setupLogsEventListeners(); await loadInitialLogs(); }

window.addEventListener("beforeunload", () => {});