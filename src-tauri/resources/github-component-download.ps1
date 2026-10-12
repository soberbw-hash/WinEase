$ErrorActionPreference='Stop'
$request=[Net.HttpWebRequest]::Create($url);$request.UserAgent='WinEase';$request.Timeout=30000;$request.ReadWriteTimeout=30000;$request.MaximumAutomaticRedirections=5
$response=$request.GetResponse()
try {
 if($response.ResponseUri.Scheme -ne 'https'){throw '禁止降级为不安全下载'}
 $inputStream=$response.GetResponseStream();$file=[IO.File]::Open($destination,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try {$buffer=New-Object byte[] 65536;$count=0L;while(($read=$inputStream.Read($buffer,0,$buffer.Length)) -gt 0){$count+=$read;if($count -gt $expectedSize -or $count -gt 209715200){throw '下载超过资源大小上限'};$file.Write($buffer,0,$read)};$file.Flush($true)}finally{$file.Dispose();$inputStream.Dispose()}
 if($count -ne $expectedSize){throw '下载大小不完整'}
 $hashAlgorithm=[Security.Cryptography.SHA256]::Create();$hashStream=[IO.File]::OpenRead($destination)
 try {$actualHash=[BitConverter]::ToString($hashAlgorithm.ComputeHash($hashStream)).Replace('-','').ToLowerInvariant()}finally{$hashStream.Dispose();$hashAlgorithm.Dispose()}
 if($actualHash -ne $expected){throw '官方资源 SHA-256 校验失败'}
} finally {$response.Dispose()}
if($url.EndsWith('.zip')) {
 Add-Type -AssemblyName System.IO.Compression.FileSystem
 $zip=[IO.Compression.ZipFile]::OpenRead($destination)
 try {
  $base=[IO.Path]::GetFullPath($payload).TrimEnd('\')+'\';$bytes=0L
  if($zip.Entries.Count -gt 50000){throw '压缩包条目过多'}
  foreach($entry in $zip.Entries){
   $target=[IO.Path]::GetFullPath((Join-Path $payload $entry.FullName));$bytes+=$entry.Length
   if(!$target.StartsWith($base,[StringComparison]::OrdinalIgnoreCase) -or $entry.FullName.Contains(':') -or (($entry.ExternalAttributes -shr 16) -band 61440) -eq 40960 -or ($entry.ExternalAttributes -band 1024) -ne 0 -or $bytes -gt 1073741824){throw '压缩包包含越界路径、链接或过多数据'}
  }
 } finally {$zip.Dispose()}
 [IO.Compression.ZipFile]::ExtractToDirectory($destination,$payload)
}
'verified'
