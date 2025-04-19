use std::process::Command;
use regex;

pub mod commands {
    use super::*;

    #[tauri::command]
    pub fn greet(name: &str) -> String {
        format!("Hello, {}! You've been greeted from Rust!", name)
    }

    #[derive(serde::Serialize, Clone)]
    pub struct AdapterInfo {
        pub name: String,
        pub status: String,
    }

    #[tauri::command]
    pub fn get_network_adapters() -> Result<Vec<AdapterInfo>, String> {
        // 使用UTF-8编码输出，并添加更多信息以便更好地显示
        let output = Command::new("powershell")
            .args(["-Command", "$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-NetAdapter | Select-Object @{Name='Name';Expression={$_.Name}}, @{Name='Status';Expression={$_.Status}}, @{Name='DisplayName';Expression={$_.InterfaceDescription}} | ConvertTo-Json -Compress"])
            .output()
            .map_err(|e| format!("执行powershell命令失败: {}", e))?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        // 确保使用UTF-8解码输出
        let txt = String::from_utf8_lossy(&output.stdout);

        // 尝试解析为数组
        if let Ok(json_adapters) = serde_json::from_str::<Vec<serde_json::Value>>(&txt) {
            let adapters = json_adapters.into_iter()
                .filter_map(|adapter| {
                    let name = adapter["Name"].as_str()?;
                    let status = adapter["Status"].as_str()?;
                    // 使用DisplayName作为备用，如果Name显示为乱码
                    let display_name = adapter["DisplayName"].as_str().unwrap_or(name);

                    // 构建一个更友好的显示名称
                    let friendly_name = if name.contains("\u{fffd}") { // 检测Unicode替换字符（常见于乱码）
                        format!("{} ({})", display_name, status)
                    } else {
                        format!("{} ({})", name, status)
                    };

                    Some(AdapterInfo {
                        name: name.to_string(),
                        status: friendly_name,
                    })
                })
                .collect();
            return Ok(adapters);
        }

        // 尝试解析为单个对象
        if let Ok(adapter) = serde_json::from_str::<serde_json::Value>(&txt) {
            if let (Some(name), Some(status)) = (adapter["Name"].as_str(), adapter["Status"].as_str()) {
                let display_name = adapter["DisplayName"].as_str().unwrap_or(name);
                let friendly_name = if name.contains("\u{fffd}") {
                    format!("{} ({})", display_name, status)
                } else {
                    format!("{} ({})", name, status)
                };

                return Ok(vec![AdapterInfo {
                    name: name.to_string(),
                    status: friendly_name,
                }]);
            }
        }

        Err(format!("无法解析网络适配器信息: {}", txt))
    }

    #[derive(serde::Deserialize, serde::Serialize, Clone)]
    pub struct Ipv4Config {
        pub adapter: String,
        pub ip: String,
        pub mask: String,
        pub gateway: String,
        pub dns1: String,
        pub dns2: String,
    }

    #[tauri::command]
    pub fn get_current_config(adapter_name: String) -> Result<Ipv4Config, String> {
        if adapter_name.trim().is_empty() {
            return Err("请选择一个网络适配器".to_string());
        }

        // 获取IP地址和子网掩码
        let get_ip_cmd = format!(
            "Get-NetIPAddress -InterfaceAlias '{}' -AddressFamily IPv4 | Select-Object IPAddress, PrefixLength | ConvertTo-Json -Compress",
            adapter_name
        );

        let ip_output = Command::new("powershell")
            .args(["-Command", &get_ip_cmd])
            .output()
            .map_err(|e| format!("执行powershell命令失败: {}", e))?;

        if !ip_output.status.success() {
            return Err(format!("获取IP配置失败: {}",
                            String::from_utf8_lossy(&ip_output.stderr)));
        }

        let ip_json = String::from_utf8_lossy(&ip_output.stdout).to_string();
        let ip_info: serde_json::Value = serde_json::from_str(&ip_json)
            .map_err(|e| format!("解析IP信息失败: {}", e))?;

        let ip_address = match ip_info["IPAddress"].as_str() {
            Some(ip) => ip.to_string(),
            None => return Err("无法获取IP地址".to_string()),
        };

        let prefix_length = match ip_info["PrefixLength"].as_u64() {
            Some(prefix) => prefix as u8,
            None => return Err("无法获取子网前缀长度".to_string()),
        };

        // 将子网前缀长度转换为子网掩码
        let mask = match prefix_to_subnet_mask(prefix_length) {
            Some(m) => m,
            None => return Err(format!("无效的子网前缀长度: {}", prefix_length)),
        };

        // 获取网关
        let get_gateway_cmd = format!(
            "Get-NetRoute -InterfaceAlias '{}' -DestinationPrefix '0.0.0.0/0' | Select-Object -ExpandProperty NextHop",
            adapter_name
        );

        let gateway_output = Command::new("powershell")
            .args(["-Command", &get_gateway_cmd])
            .output()
            .map_err(|e| format!("执行powershell命令失败: {}", e))?;

        let gateway = if gateway_output.status.success() {
            String::from_utf8_lossy(&gateway_output.stdout).trim().to_string()
        } else {
            "".to_string() // 如果没有网关，返回空字符串
        };

        // 获取DNS服务器
        let get_dns_cmd = format!(
            "Get-DnsClientServerAddress -InterfaceAlias '{}' -AddressFamily IPv4 | Select-Object -ExpandProperty ServerAddresses",
            adapter_name
        );

        let dns_output = Command::new("powershell")
            .args(["-Command", &get_dns_cmd])
            .output()
            .map_err(|e| format!("执行powershell命令失败: {}", e))?;

        let dns_servers = if dns_output.status.success() {
            let dns_text = String::from_utf8_lossy(&dns_output.stdout).to_string();
            let servers: Vec<String> = dns_text.lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .take(2) // 只取前两个DNS
                .collect();
            servers
        } else {
            vec![]
        };

        let dns1 = dns_servers.get(0).cloned().unwrap_or_default();
        let dns2 = dns_servers.get(1).cloned().unwrap_or_default();

        Ok(Ipv4Config {
            adapter: adapter_name,
            ip: ip_address,
            mask,
            gateway,
            dns1,
            dns2,
        })
    }

    #[tauri::command]
    pub fn apply_adapter_ipv4_config(cfg: Ipv4Config) -> Result<String, String> {
        // 验证适配器名称是否为空
        if cfg.adapter.trim().is_empty() {
            return Err("请选择一个网络适配器".to_string());
        }

        // 验证必填IP地址格式
        if cfg.ip.trim().is_empty() {
            return Err("IP地址不能为空".to_string());
        }

        if !is_valid_ip(&cfg.ip) {
            return Err("IP地址格式不正确，请使用xxx.xxx.xxx.xxx格式".to_string());
        }

        if cfg.mask.trim().is_empty() {
            return Err("子网掩码不能为空".to_string());
        }

        if !is_valid_ip(&cfg.mask) {
            return Err("子网掩码格式不正确，请使用xxx.xxx.xxx.xxx格式".to_string());
        }

        // 验证可选字段格式
        if !is_valid_ip(&cfg.gateway) {
            return Err("网关地址格式不正确，请使用xxx.xxx.xxx.xxx格式或留空".to_string());
        }

        if !is_valid_ip(&cfg.dns1) {
            return Err("DNS1地址格式不正确，请使用xxx.xxx.xxx.xxx格式或留空".to_string());
        }

        if !is_valid_ip(&cfg.dns2) {
            return Err("DNS2地址格式不正确，请使用xxx.xxx.xxx.xxx格式或留空".to_string());
        }

        // 设置IP地址、子网掩码和网关
        let set_ip_cmd = if cfg.gateway.trim().is_empty() {
            // 如果网关为空，不设置网关
            format!(
                "netsh interface ip set address name=\"{}\" static {} {}",
                cfg.adapter, cfg.ip, cfg.mask
            )
        } else {
            format!(
                "netsh interface ip set address name=\"{}\" static {} {} {}",
                cfg.adapter, cfg.ip, cfg.mask, cfg.gateway
            )
        };

        let set_ip = Command::new("powershell")
            .args(["-Command", &set_ip_cmd])
            .output()
            .map_err(|e| format!("执行powershell命令失败: {}", e))?;

        if !set_ip.status.success() {
            let error = String::from_utf8_lossy(&set_ip.stderr).to_string();
            return Err(format!("设置IP地址失败: {}\n命令: {}", error, set_ip_cmd));
        }

        // 设置DNS服务器
        if !cfg.dns1.trim().is_empty() {
            // 设置主DNS服务器
            let set_dns1_cmd = format!(
                "netsh interface ip set dns name=\"{}\" static {}",
                cfg.adapter, cfg.dns1
            );

            let set_dns1 = Command::new("powershell")
                .args(["-Command", &set_dns1_cmd])
                .output()
                .map_err(|e| format!("执行powershell命令失败: {}", e))?;

            if !set_dns1.status.success() {
                let error = String::from_utf8_lossy(&set_dns1.stderr).to_string();
                return Err(format!("设置主DNS服务器失败: {}\n命令: {}", error, set_dns1_cmd));
            }

            // 如果有辅助DNS，则设置
            if !cfg.dns2.trim().is_empty() {
                let set_dns2_cmd = format!(
                    "netsh interface ip add dns name=\"{}\" {} index=2",
                    cfg.adapter, cfg.dns2
                );

                let set_dns2 = Command::new("powershell")
                    .args(["-Command", &set_dns2_cmd])
                    .output()
                    .map_err(|e| format!("执行powershell命令失败: {}", e))?;

                if !set_dns2.status.success() {
                    let error = String::from_utf8_lossy(&set_dns2.stderr).to_string();
                    return Err(format!("设置辅助DNS服务器失败: {}\n命令: {}", error, set_dns2_cmd));
                }
            }
        }

        Ok("IPv4 配置已成功应用".to_string())
    }
}

// 将子网前缀长度转换为子网掩码
fn prefix_to_subnet_mask(prefix_length: u8) -> Option<String> {
    if prefix_length > 32 {
        return None;
    }

    let mask_bits: u32 = if prefix_length == 0 {
        0
    } else {
        !0u32 << (32 - prefix_length)
    };

    let octet1 = (mask_bits >> 24) & 0xff;
    let octet2 = (mask_bits >> 16) & 0xff;
    let octet3 = (mask_bits >> 8) & 0xff;
    let octet4 = mask_bits & 0xff;

    Some(format!("{}.{}.{}.{}", octet1, octet2, octet3, octet4))
}

/// 验证IP地址是否有效，允许空字符串
fn is_valid_ip(ip: &str) -> bool {
    // 允许空字符串作为有效输入（用于可选的网关和DNS）
    if ip.trim().is_empty() {
        return true;
    }

    // 使用lazy_static避免每次调用都重新编译正则表达式
    use std::sync::OnceLock;
    static IP_REGEX: OnceLock<regex::Regex> = OnceLock::new();

    let regex = IP_REGEX.get_or_init(|| {
        regex::Regex::new(r"^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$").unwrap()
    });

    regex.is_match(ip)
}

/// 启动Tauri应用程序
#[allow(dead_code)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::get_network_adapters,
            commands::apply_adapter_ipv4_config,
            commands::get_current_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
