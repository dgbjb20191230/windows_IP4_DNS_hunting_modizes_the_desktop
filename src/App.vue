/// <reference types="@tauri-apps/api" />
<script setup lang="ts">
import { ref, onMounted, reactive } from "vue";
import { invoke } from '@tauri-apps/api/tauri';

interface AdapterInfo {
  name: string;
  status: string;
}

interface IpConfig {
  adapter: string;
  ip: string;
  mask: string;
  gateway: string;
  dns1: string;
  dns2: string;
}

const adapters = ref<AdapterInfo[]>([]);
const selectedAdapter = ref("");
const ipConfig = reactive<IpConfig>({
  adapter: "",
  ip: "",
  mask: "",
  gateway: "",
  dns1: "",
  dns2: ""
});
const statusMsg = ref("");
const configList = ref<IpConfig[]>([]);
const isLoading = ref(false);

const CONFIG_KEY = "net_config_history";

async function loadAdapters() {
  isLoading.value = true;
  try {
    const result = await invoke<AdapterInfo[]>("get_network_adapters");
    adapters.value = result;
    if (result.length > 0) {
      selectedAdapter.value = result[0].name;
      ipConfig.adapter = selectedAdapter.value;
    }
  } catch (e: any) {
    statusMsg.value = "获取适配器失败：" + (e.message || e);
  } finally {
    isLoading.value = false;
  }
}

function saveConfig() {
  const newCfg: IpConfig = {
    adapter: selectedAdapter.value,
    ip: ipConfig.ip,
    mask: ipConfig.mask,
    gateway: ipConfig.gateway,
    dns1: ipConfig.dns1,
    dns2: ipConfig.dns2
  };

  // 避免重复保存
  const configJson = JSON.stringify(newCfg);
  const existingIndex = configList.value.findIndex(c =>
    JSON.stringify(c) === configJson);

  if (existingIndex !== -1) {
    // 将已存在的项移到最前面
    const item = configList.value.splice(existingIndex, 1)[0];
    configList.value.unshift(item);
  } else {
    // 限制最大历史记录数量为10
    if (configList.value.length >= 10) {
      configList.value.pop();
    }
    configList.value.unshift(newCfg);
  }

  localStorage.setItem(CONFIG_KEY, JSON.stringify(configList.value));
  statusMsg.value = "配置已保存";
}

function loadConfigList() {
  try {
    const str = localStorage.getItem(CONFIG_KEY);
    if (str) {
      const parsed = JSON.parse(str);
      if (Array.isArray(parsed)) {
        configList.value = parsed;
      }
    }
  } catch (err) {
    console.error("加载配置历史记录失败", err);
    // 静默失败，不影响用户体验
  }
}

function validateIpAddress(ip: string): boolean {
  // 允许空字符串（用于可选字段）
  if (ip.trim() === "") {
    return true;
  }

  // 使用更简洁的正则表达式
  const ipParts = ip.split('.');

  // 检查是否有四个部分
  if (ipParts.length !== 4) {
    return false;
  }

  // 检查每个部分是否是0-255的数字
  return ipParts.every(part => {
    const num = parseInt(part, 10);
    return !isNaN(num) && num >= 0 && num <= 255 && part === num.toString();
  });
}

async function applyConfig() {
  // 验证必填字段
  if (ipConfig.ip.trim() === "") {
    statusMsg.value = "IP地址不能为空";
    return;
  }

  if (ipConfig.mask.trim() === "") {
    statusMsg.value = "子网掩码不能为空";
    return;
  }

  // 验证格式
  if (!validateIpAddress(ipConfig.ip)) {
    statusMsg.value = "IP地址格式不正确，请使用xxx.xxx.xxx.xxx格式";
    return;
  }

  if (!validateIpAddress(ipConfig.mask)) {
    statusMsg.value = "子网掩码格式不正确，请使用xxx.xxx.xxx.xxx格式";
    return;
  }

  // 验证可选字段格式
  if (!validateIpAddress(ipConfig.gateway)) {
    statusMsg.value = "网关地址格式不正确，请使用xxx.xxx.xxx.xxx格式或留空";
    return;
  }

  if (!validateIpAddress(ipConfig.dns1)) {
    statusMsg.value = "DNS1地址格式不正确，请使用xxx.xxx.xxx.xxx格式或留空";
    return;
  }

  if (!validateIpAddress(ipConfig.dns2)) {
    statusMsg.value = "DNS2地址格式不正确，请使用xxx.xxx.xxx.xxx格式或留空";
    return;
  }

  statusMsg.value = "正在应用配置...";
  isLoading.value = true;

  try {
    ipConfig.adapter = selectedAdapter.value;
    const msg = await invoke<string>("apply_adapter_ipv4_config", {
      cfg: ipConfig,
    });
    statusMsg.value = msg;
    saveConfig();
  } catch (e: any) {
    statusMsg.value = "应用失败：" + (e.message || e);
  } finally {
    isLoading.value = false;
  }
}

function fillFromHistory(cfg: IpConfig) {
  selectedAdapter.value = cfg.adapter;
  ipConfig.ip = cfg.ip;
  ipConfig.mask = cfg.mask;
  ipConfig.gateway = cfg.gateway;
  ipConfig.dns1 = cfg.dns1;
  ipConfig.dns2 = cfg.dns2;
}

async function getCurrentConfig() {
  if (!selectedAdapter.value) {
    statusMsg.value = "请先选择一个网络适配器";
    return;
  }

  statusMsg.value = "正在获取当前配置...";
  isLoading.value = true;

  try {
    const config = await invoke<IpConfig>("get_current_config", {
      adapterName: selectedAdapter.value
    });

    ipConfig.ip = config.ip || "";
    ipConfig.mask = config.mask || "";
    ipConfig.gateway = config.gateway || "";
    ipConfig.dns1 = config.dns1 || "";
    ipConfig.dns2 = config.dns2 || "";
    statusMsg.value = "已获取当前配置";
  } catch (e: any) {
    statusMsg.value = "获取当前配置失败：" + (e.message || e);
  } finally {
    isLoading.value = false;
  }
}

onMounted(() => {
  loadAdapters();
  loadConfigList();
});
</script>

<template>
  <main class="container">
    <h1>Windows IPv4 配置工具</h1>

    <div class="adapter-selector">
      <select v-model="selectedAdapter" :disabled="isLoading">
        <option v-for="a in adapters" :key="a.name" :value="a.name">
          {{ a.status }}
        </option>
      </select>
    </div>

    <form @submit.prevent="applyConfig" class="config-form">
      <input v-model="ipConfig.ip" placeholder="IP 地址 (如 192.168.1.100)" required :disabled="isLoading" />
      <input v-model="ipConfig.mask" placeholder="子网掩码 (如 255.255.255.0)" required :disabled="isLoading" />
      <input v-model="ipConfig.gateway" placeholder="网关 (如 192.168.1.1)" required :disabled="isLoading" />
      <input v-model="ipConfig.dns1" placeholder="DNS1 (如 8.8.8.8)" required :disabled="isLoading" />
      <input v-model="ipConfig.dns2" placeholder="DNS2 (如 8.8.4.4)" required :disabled="isLoading" />

      <div class="button-group">
        <button type="button" @click="getCurrentConfig" :disabled="isLoading">获取当前配置</button>
        <button type="button" @click="saveConfig" :disabled="isLoading">保存配置</button>
        <button type="submit" :disabled="isLoading">应用配置</button>
      </div>
    </form>

    <p class="status-message" :class="{ error: statusMsg.includes('失败') }">
      {{ statusMsg }}
      <span v-if="isLoading" class="loading-indicator">处理中...</span>
    </p>

    <div v-if="configList.length" class="history-section">
      <h3>历史配置</h3>
      <ul>
        <li v-for="(cfg, idx) in configList" :key="idx">
          <button type="button" @click="fillFromHistory(cfg)" :disabled="isLoading">应用</button>
          <span class="config-summary">
            {{ cfg.adapter }} | {{ cfg.ip }} | {{ cfg.gateway }}
          </span>
        </li>
      </ul>
    </div>
  </main>
</template>

<style scoped>
.container {
  margin: 0 auto;
  max-width: 800px;
  padding: 2rem;
  display: flex;
  flex-direction: column;
  align-items: center;
}

h1 {
  color: #0f0f0f;
  margin-bottom: 2rem;
}

.adapter-selector {
  margin-bottom: 1.5rem;
  width: 100%;
  max-width: 400px;
}

select {
  width: 100%;
  padding: 0.6rem;
  border-radius: 8px;
  border: 1px solid #ddd;
  font-size: 1rem;
}

.config-form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  width: 100%;
  max-width: 400px;
}

input {
  padding: 0.6rem;
  border-radius: 8px;
  border: 1px solid #ddd;
  font-size: 1rem;
}

input:focus, select:focus {
  outline: none;
  border-color: #24c8db;
}

.button-group {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.5rem;
  justify-content: center;
}

button {
  background-color: #24c8db;
  color: white;
  border: none;
  border-radius: 8px;
  padding: 0.6rem 1rem;
  font-size: 0.9rem;
  cursor: pointer;
  transition: background-color 0.2s;
}

button:hover {
  background-color: #1eafc0;
}

button:disabled {
  background-color: #cccccc;
  cursor: not-allowed;
}

.status-message {
  margin-top: 1rem;
  color: #24c8db;
  min-height: 24px;
}

.status-message.error {
  color: #e53935;
}

.loading-indicator {
  margin-left: 0.5rem;
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0% { opacity: 0.5; }
  50% { opacity: 1; }
  100% { opacity: 0.5; }
}

.history-section {
  margin-top: 2rem;
  width: 100%;
  max-width: 600px;
}

.history-section h3 {
  margin-bottom: 0.5rem;
  color: #333;
}

.history-section ul {
  list-style: none;
  padding: 0;
  margin: 0;
}

.history-section li {
  display: flex;
  align-items: center;
  margin-bottom: 0.5rem;
  padding: 0.5rem;
  border-radius: 8px;
  background-color: #f5f5f5;
}

.history-section button {
  margin-right: 0.75rem;
  padding: 0.3rem 0.6rem;
  font-size: 0.8rem;
}

.config-summary {
  font-size: 0.9rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>