Add-Type -AssemblyName System.Net.Http
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$endpoints = '__ENDPOINTS_JSON__' | ConvertFrom-Json
$results = [Collections.Generic.List[object]]::new()
function Test-Http([string]$id,[string]$name,[string]$target,[string]$mode) {
 $watch=[Diagnostics.Stopwatch]::StartNew(); $handler=[Net.Http.HttpClientHandler]::new();$handler.AllowAutoRedirect=$false
 if($mode -eq 'direct') {$handler.UseProxy=$false} elseif($mode -eq 'system') {$handler.UseProxy=$true;$handler.Proxy=[Net.WebRequest]::GetSystemWebProxy()} else {$handler.UseProxy=$true;$handler.Proxy=[Net.WebProxy]::new($mode)}
 $client=[Net.Http.HttpClient]::new($handler);$client.Timeout=[TimeSpan]::FromSeconds(5)
 try {
  $response=$client.GetAsync($target,[Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult();$code=[int]$response.StatusCode
  $ok=$code -ge 200 -and $code -lt 400;$portal=$false
  if($id -eq 'portal') { $text=$response.Content.ReadAsStringAsync();if(!$text.Wait(2000)){throw '联网验证内容超时'};$ok=$code -eq 200 -and $text.Result.Trim() -eq 'Microsoft Connect Test';$portal=!$ok -and ($code -ge 200 -and $code -lt 400) }
  $results.Add([ordered]@{id=$id;name=$name;target=$target;ok=$ok;reachable=$true;portal=$portal;latencyMs=$watch.ElapsedMilliseconds;detail=('HTTP '+$code)});$response.Dispose()
 } catch {$results.Add([ordered]@{id=$id;name=$name;target=$target;ok=$false;reachable=$false;portal=$false;latencyMs=$watch.ElapsedMilliseconds;detail=$_.Exception.GetBaseException().Message})}
 finally {$client.Dispose();$handler.Dispose()}
}
Test-Http 'direct-cn' '直连 · 百度' 'https://www.baidu.com/' 'direct'
Test-Http 'direct-ms' '直连 · Microsoft' 'https://www.microsoft.com/' 'direct'
Test-Http 'system-cn' '系统连接 · 百度' 'https://www.baidu.com/' 'system'
Test-Http 'system-ms' '系统连接 · Microsoft' 'https://www.microsoft.com/' 'system'
Test-Http 'portal' '联网验证' 'http://www.msftconnecttest.com/connecttest.txt' 'direct'
foreach($endpoint in @($endpoints)) {
 $tcp=[Net.Sockets.TcpClient]::new();$watch=[Diagnostics.Stopwatch]::StartNew();$ok=$false;$detail=''
 try {$task=$tcp.ConnectAsync([string]$endpoint.host,[int]$endpoint.port);if(!$task.Wait(1500)){throw '连接超时'};$ok=$tcp.Connected;$detail='端口可连接'}catch{$detail=$_.Exception.GetBaseException().Message}finally{$tcp.Dispose()}
 $results.Add([ordered]@{id=('local-'+$endpoint.port+'-'+$endpoint.host);name='本地代理端口';target=($endpoint.host+':'+$endpoint.port);ok=$ok;reachable=$ok;portal=$false;latencyMs=$watch.ElapsedMilliseconds;detail=$detail})
}
# A listening SOCKS port is not an HTTP proxy. Only probe a verified HTTP/HTTPS mapping.
$httpProxy='__HTTP_PROXY__'
if($httpProxy){Test-Http 'proxy-egress' '代理出口 · GitHub' 'https://github.com/' $httpProxy}
ConvertTo-Json -InputObject @($results) -Depth 8 -Compress
