# Run against isolated fixtures on the Windows interactive desktop.
# This uses a Windows loopback client; it does not certify the macOS network path.
param(
 [Parameter(Mandatory=$true)][string]$Bridge,
 [Parameter(Mandatory=$true)][string]$Pairing,
 [string]$Address='127.0.0.1:47832',
 [Parameter(Mandatory=$true)][string]$Results,
 [switch]$StressRapidSwitch
)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$script:targets=@{}
function Start-Bridge {
 $info=[Diagnostics.ProcessStartInfo]::new($Bridge,"$Address `"$Pairing`" --bridge")
 $info.UseShellExecute=$false
 $info.RedirectStandardInput=$true
 $info.RedirectStandardOutput=$true
 $info.RedirectStandardError=$true
 $info.StandardOutputEncoding=[Text.Encoding]::UTF8
 $info.StandardErrorEncoding=[Text.Encoding]::UTF8
 $script:child=[Diagnostics.Process]::Start($info)

}
function Stop-Bridge {
 if ($script:child) {
  $script:child.StandardInput.BaseStream.Close()
  if (!$script:child.WaitForExit(5000)) {$script:child.Kill();$script:child.WaitForExit()}
  $script:child.Dispose();$script:child=$null
 }
}
function Call([int]$sid,[string]$command,[hashtable]$fields=@{},[switch]$AllowError) {
 $fields.command=$command;$fields.session_id=$sid
 $json=($fields | ConvertTo-Json -Depth 12 -Compress)+"`n"
 $bytes=[Text.UTF8Encoding]::new($false).GetBytes($json)
 $script:child.StandardInput.BaseStream.Write($bytes,0,$bytes.Length)
 $script:child.StandardInput.BaseStream.Flush()
 $read=$script:child.StandardOutput.ReadLineAsync()
 if (!$read.Wait(15000)) {throw "Response timeout: $command"}
 if (!$read.Result) {throw 'Bridge closed unexpectedly'}
 $reply=$read.Result | ConvertFrom-Json
 if ($reply.type -eq 'bridge_error') {throw "Bridge failure in $command`: $($reply.category), $($reply.message)"}
 if (!$AllowError -and $reply.type -eq 'error') {throw "Command failed in session $sid`: $command ($($reply.message))"}
 return $reply
}
function Select-Fixture([int]$sid,[string]$title) {
 $window=$null
 for($i=0;$i -lt 30 -and !$window;$i++) {
  $window=(Call 0 list_windows).windows | Where-Object title -eq $title | Select-Object -First 1
  if (!$window) {Start-Sleep -Milliseconds 200}
 }
 if (!$window) {throw "Missing synthetic fixture: $title"}
 $selected=Call $sid select_window @{id=$window.id;scope='application'}
 $group=Call $sid window_group
 $target=$group.windows | Where-Object id -eq $window.id | Select-Object -First 1
 $script:targets[$sid]=@{id=$window.id;token=$target.token;ticket=$selected.resume_token;bounds=$group.root_bounds}
}
function Input-Events([int]$sid,[object[]]$events) {
 $t=$script:targets[$sid]
 $null=Call $sid input @{id=$t.id;token=$t.token;events=$events}
}
function Key-Pair([int]$vk) {return @(@{kind='key';vk=$vk;down=$true},@{kind='key';vk=$vk;down=$false})}
function Click-Fixture([int]$sid) {
 $b=$script:targets[$sid].bounds
 Input-Events $sid @(@{kind='button';x=.5;y=.4;width=($b.right-$b.left);height=($b.bottom-$b.top);button=0;down=$true},@{kind='button';x=.5;y=.4;width=($b.right-$b.left);height=($b.bottom-$b.top);button=0;down=$false})
}
function Select-All([int]$sid) {Input-Events $sid (@(@{kind='key';vk=17;down=$true})+(Key-Pair 65)+@(@{kind='key';vk=17;down=$false}))}
function Clipboard-Action([int]$sid,[string]$action,[hashtable]$payload=@{},[switch]$AllowError) {
 $t=$script:targets[$sid];$payload.id=$t.id;$payload.token=$t.token;$payload.action=$action
 return Call $sid clipboard $payload -AllowError:$AllowError
}
function Wait-For([scriptblock]$predicate,[string]$failure) {
 for($i=0;$i -lt 50;$i++) {if (& $predicate) {return};Start-Sleep -Milliseconds 100}
 throw $failure
}
function Read-Result([string]$file) {return [IO.File]::ReadAllText((Join-Path $Results $file)).Replace("`r`n","`n")}
Start-Bridge
try {
 Select-Fixture 1 'Kokoda Multi A Test';Select-Fixture 2 'Kokoda Multi B Test'
 $unicode=[string][char]0x4E2D+[char]0x6587
 foreach($sid in @(1,2)) {
  Click-Fixture $sid;Select-All $sid
  Input-Events $sid @(@{kind='text';text="Kokoda multi $sid $unicode"})
  if (!$StressRapidSwitch) {
   $file=if($sid -eq 1){'multi-a.txt'}else{'multi-b.txt'}
   Wait-For {(Read-Result $file) -eq "Kokoda multi $sid $unicode"} 'Unicode text did not reach its target'
  }
 }
 Wait-For { (Read-Result 'multi-a.txt') -eq "Kokoda multi 1 $unicode" -and (Read-Result 'multi-b.txt') -eq "Kokoda multi 2 $unicode" } 'Unicode input crossed application sessions'
 Input-Events 1 @(@{kind='key';vk=16;down=$true})
 Click-Fixture 2;Input-Events 2 ((Key-Pair 35)+(Key-Pair 65))
 Wait-For { (Read-Result 'multi-b.txt').EndsWith('a') } 'Shift leaked from the first application'
 Input-Events 2 @(@{kind='key';vk=16;down=$true})
 Input-Events 1 @(@{kind='release'})
 Input-Events 2 ((Key-Pair 65)+@(@{kind='release'}))
 Wait-For { (Read-Result 'multi-b.txt').EndsWith('aA') } 'Late release from the first application affected the second'
 $wrong=Call 1 input @{id=$targets[2].id;token=$targets[2].token;events=@(@{kind='text';text='must not arrive'})} -AllowError
 if($wrong.type -ne 'error') {throw 'Cross-session target accepted'}
 Stop-Bridge;Start-Bridge
 foreach($sid in @(1,2)) {
  $null=Call $sid resume_window @{resume_token=$targets[$sid].ticket}
  $g=Call $sid window_group
  $targets[$sid].token=($g.windows | Where-Object id -eq $targets[$sid].id).token
 }
 $null=Call 1 close_window
 Input-Events 2 @(@{kind='text';text=' still live'})
 Wait-For {(Read-Result 'multi-b.txt').EndsWith(' still live')} 'Closing one session disrupted the other'
 $null=Call 2 close_window
 Write-Output 'PASS: loopback two-app Unicode, modifier isolation, stale release, target checks, resume and independent close'
 Select-Fixture 3 'Kokoda Clipboard Test';Click-Fixture 3;Select-All 3
 $sample="Kokoda clipboard $unicode`nSecond line"
 $null=Clipboard-Action 3 paste @{text=$sample}
 Wait-For {(Read-Result 'clipboard-result.txt') -eq $sample} 'Multiline clipboard paste failed'
 Select-All 3;$copied=Clipboard-Action 3 copy
 if ($copied.text.Replace("`r`n","`n") -ne $sample) {throw 'Clipboard copy changed text'}
 $null=Clipboard-Action 3 cut
 Wait-For {(Read-Result 'clipboard-result.txt') -eq ''} 'Clipboard cut failed'
 $null=Clipboard-Action 3 paste @{text=$sample}
 $t=$targets[3]
 $bad=Call 3 clipboard @{id=$t.id;token=($t.token+999);action='paste';text='must not arrive'} -AllowError
 if ($bad.type -ne 'error') {throw 'Stale clipboard token accepted'}
 $null=Call 3 close_window
 Write-Output 'PASS: loopback multiline Unicode clipboard, copy, cut, paste and stale-target rejection'
 Select-Fixture 4 'Kokoda Image Clipboard Test'
 $png='iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP4z8DwHwgbGMA0EAAAP9cIeY8TiooAAAAASUVORK5CYII='
 $null=Clipboard-Action 4 paste @{image_png=$png}
 Wait-For {((Read-Result 'image-result.json') | ConvertFrom-Json).width -eq 2} 'Image clipboard paste failed'
 $image=(Read-Result 'image-result.json') | ConvertFrom-Json
 if($image.height -ne 2 -or $image.pixels[0][0] -ne 255 -or $image.pixels[1][3] -ne 128 -or $image.pixels[2][3] -ne 0) {throw 'Image dimensions, pixels or alpha changed'}
 $copied=Clipboard-Action 4 copy
 $stream=[IO.MemoryStream]::new([Convert]::FromBase64String($copied.image_png))
 $bitmap=[Drawing.Bitmap]::new($stream)
 try {if($bitmap.Width -ne 2 -or $bitmap.GetPixel(1,0).A -ne 128) {throw 'Copied image changed alpha'}} finally {$bitmap.Dispose();$stream.Dispose()}
 $bad=Clipboard-Action 4 paste @{image_png='AQID'} -AllowError
 if($bad.type -ne 'error') {throw 'Malformed image accepted'}
 $null=Call 4 close_window
 Write-Output 'PASS: loopback PNG paste/copy, pixel and alpha preservation, malformed payload rejection'
} finally {Stop-Bridge}
