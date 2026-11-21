param([Parameter(Mandatory=$true)][string]$BinaryDirectory,[Parameter(Mandatory=$true)][string]$ResultDirectory)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName Accessibility
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class HostGuiNative {
 public static IntPtr WindowPicker() {return FindWindow("KokodaWindowPicker",null);}
 public static IntPtr PasswordDialog() {return FindWindow("KokodaPasswordDialog",null);}
 public delegate bool EnumProc(IntPtr hwnd,IntPtr param);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback,IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder text,int len);
 public static string TestWindows(uint process) {var list=new System.Collections.Generic.List<string>();EnumWindows((h,p)=>{uint id;GetWindowThreadProcessId(h,out id);if(id==process){var name=new StringBuilder(256);GetClassName(h,name,256);list.Add(name.ToString()+":"+h+":edit="+GetDlgItem(h,202));}return true;},IntPtr.Zero);return string.Join(";",list);}

 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls,string title);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
 [DllImport("oleacc.dll")] public static extern int AccessibleObjectFromWindow(IntPtr h,uint id,ref Guid iid,[MarshalAs(UnmanagedType.Interface)] out object result);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h,int id);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageW")] public static extern IntPtr WriteText(IntPtr h,uint m,IntPtr w,string s);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageW")] public static extern IntPtr ReadText(IntPtr h,uint m,IntPtr w,StringBuilder text);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int L,T,R,B;}
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);
}
'@
# PE optional-header Subsystem must be Windows GUI (2), not Console (3).
$pe=[IO.File]::ReadAllBytes((Join-Path $BinaryDirectory 'KokodaHost.exe'))
$offset=[BitConverter]::ToInt32($pe,0x3c)
if([BitConverter]::ToUInt16($pe,$offset+24+68) -ne 2){throw 'Executable has a console subsystem'}
New-Item -ItemType Directory -Force $ResultDirectory | Out-Null
$data=Join-Path $ResultDirectory ('identity-test-'+[Guid]::NewGuid().ToString('N'))
$env:KOKODA_HOST_DATA_DIR=$data
$gui=$null;$bridge=$null;$listener=$null;$workerId=$null
$fixtures=@();$originalComputerName=$env:COMPUTERNAME
$env:COMPUTERNAME="Kokoda-Discovery-Gui-Fixture"
function Assert($value,$message) { if(!$value){throw $message} }
function Control($id) { [HostGuiNative]::GetDlgItem($script:window,$id) }
function Click($id) { [HostGuiNative]::SendMessage((Control $id),0xF5,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null }
function Set-Field($id,$value) {[HostGuiNative]::WriteText((Control $id),0xC,[IntPtr]::Zero,$value) | Out-Null}
function Get-Field($id) {$s=New-Object Text.StringBuilder 8192;[HostGuiNative]::ReadText((Control $id),0xD,[IntPtr]$s.Capacity,$s)|Out-Null;$s.ToString()}
function Wait-For($condition,$message) {for($i=0;$i -lt 100;$i++){if(&$condition){return};Start-Sleep -Milliseconds 100};throw $message}
function Open-Gui {
 $script:gui=Start-Process "$BinaryDirectory\KokodaHost.exe" -PassThru
 Wait-For {$script:gui.Refresh();$script:gui.MainWindowHandle -ne [IntPtr]::Zero} 'GUI did not open'
 $script:window=$script:gui.MainWindowHandle
}
function Wait-Running {
 Wait-For {(Get-Field 114) -match 'Waiting for a client|等待客户端连接'} 'Host did not become ready'
 $child=Get-CimInstance Win32_Process | Where-Object {$_.ParentProcessId -eq $script:gui.Id -and $_.Name -eq 'kokoda-windows-host.exe'}
 Assert ($null -ne $child) 'Host child missing';$script:workerId=$child.ProcessId
 Assert (![HostGuiNative]::IsWindowEnabled((Control 104))) 'Scope changed while running'
}
function Wait-Stopped($id) { Wait-For {!(Get-Process -Id $id -ErrorAction SilentlyContinue)} 'Host child survived stop/GUI exit' }
function List-Windows([string]$Password=$script:password,[bool]$Reject=$false) {
 $info=New-Object Diagnostics.ProcessStartInfo
 $info.FileName="$BinaryDirectory\kokoda-bridge.exe";$info.Arguments="127.0.0.1:$script:port --password-stdin --bridge"
 $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardInput=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $script:bridge=[Diagnostics.Process]::Start($info)
 try {
  $bridge.StandardInput.WriteLine((@{password=$Password}|ConvertTo-Json -Compress));
  $bridge.StandardInput.WriteLine('{"command":"list_windows","session_id":1}');$bridge.StandardInput.Flush()
  $line=$bridge.StandardOutput.ReadLineAsync();Assert ($line.Wait(10000)) 'Authenticated GUI host request timed out'
  try {$result=$line.Result | ConvertFrom-Json} catch {throw 'Bridge returned malformed JSON (payload omitted)'}; if($Reject){Assert ($result.type -eq 'bridge_error' -and $result.category -eq 'authentication' -and !$result.retryable) 'Wrong password was accepted or retried'} else {Assert ($result.type -eq 'windows') 'Host did not return a window catalog'}
  return $result
 } finally {if(!$bridge.HasExited){$bridge.Kill()};$bridge.WaitForExit();$script:bridge=$null}
}
function Set-Password([string]$Value) {
 $before=if(Test-Path "$data\password.dat"){[Convert]::ToBase64String([IO.File]::ReadAllBytes("$data\password.dat"))}else{''}
 [HostGuiNative]::PostMessage($script:window,0x111,[IntPtr]103,(Control 103))|Out-Null
 Wait-For {([HostGuiNative]::GetDlgItem([HostGuiNative]::PasswordDialog(),202)) -ne [IntPtr]::Zero} 'Password dialog did not open'
 $dialog=[HostGuiNative]::PasswordDialog()
 foreach($id in @(201,202)){[HostGuiNative]::WriteText([HostGuiNative]::GetDlgItem($dialog,$id),0xC,[IntPtr]::Zero,$Value)|Out-Null}
 [HostGuiNative]::SendMessage([HostGuiNative]::GetDlgItem($dialog,203),0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
 Wait-For {!( [HostGuiNative]::IsWindow($dialog)) -and (Test-Path "$data\password.dat") -and [Convert]::ToBase64String([IO.File]::ReadAllBytes("$data\password.dat")) -ne $before} 'Password was not saved'
}
try {
 # Reserve an ephemeral port to exercise bind failure, then release it for real TLS requests.
 $listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Any,0);$listener.Start();$script:port=$listener.LocalEndpoint.Port
 Open-Gui
 $element=[System.Windows.Automation.AutomationElement]::FromHandle((Control 101))
 Assert ($element.Current.Name.Length -gt 0) 'Start button lacks accessibility name'
 @{'name'=$element.Current.Name;'class'=$element.Current.ClassName;'type'=$element.Current.ControlType.ProgrammaticName;'framework'=$element.Current.FrameworkId;'patterns'=@($element.GetSupportedPatterns() | ForEach-Object {$_.ProgrammaticName})} | ConvertTo-Json | Set-Content -Encoding UTF8 "$ResultDirectory\accessibility.json"
 # ARM64 UIA may lack the x64 proxy. Verify the native MSAA role/action too.
 $iid=[Guid]'618736E0-3C3D-11CF-810C-00AA00389B71';$accessible=$null
 $hr=[HostGuiNative]::AccessibleObjectFromWindow((Control 101),[uint32]4294967292,[ref]$iid,[ref]$accessible)
 Assert ($hr -eq 0) 'Start button lacks MSAA accessibility'
 $role=$accessible.accRole(0)
 Assert ($role -eq 43) 'Start button lacks native push-button role'
 Assert ($accessible.accDefaultAction(0).Length -gt 0) 'Start button lacks native accessible action'
 # The picker sees only local windows; use three synthetic same-process forms.
 $root=New-Object Windows.Forms.Form;$root.Text='Kokoda Picker Public';$root.Show()
 $other=New-Object Windows.Forms.Form;$other.Text='Kokoda Picker Private';$other.Show()
 $owned=New-Object Windows.Forms.Form;$owned.Text='Kokoda Picker Dialog';$owned.Show($root)
 $fixtures=@($owned,$other,$root);[Windows.Forms.Application]::DoEvents()
 [HostGuiNative]::PostMessage($window,0x111,[IntPtr]118,(Control 118))|Out-Null
 Wait-For {[HostGuiNative]::WindowPicker() -ne [IntPtr]::Zero} 'Window picker did not open'
 $picker=[HostGuiNative]::WindowPicker()
 $search=[HostGuiNative]::GetDlgItem($picker,301);$list=[HostGuiNative]::GetDlgItem($picker,302)
 [HostGuiNative]::WriteText($search,0xC,[IntPtr]::Zero,'Kokoda Picker Public')|Out-Null
 Wait-For {[HostGuiNative]::SendMessage($list,0x18B,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32() -eq 1} 'Picker search did not isolate fixture'
 [HostGuiNative]::SendMessage($list,0x186,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
 [HostGuiNative]::SendMessage($picker,0x111,[IntPtr](302 -bor (1 -shl 16)),$list)|Out-Null
 $preview=[HostGuiNative]::GetDlgItem($picker,304)
 Assert ([HostGuiNative]::SendMessage($preview,0x18B,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32() -eq 2) 'Preview did not distinguish owned dialog from independent sibling'
 # Automatic refresh preserves the same window selection when its title changes.
 $root.Text='Kokoda Picker Public renamed';[Windows.Forms.Application]::DoEvents()
 Wait-For {$line=New-Object Text.StringBuilder 4096;[HostGuiNative]::ReadText($list,0x189,[IntPtr]::Zero,$line)|Out-Null;$line.ToString().Contains('renamed')} 'Automatic picker refresh missed title change'
 Assert ([HostGuiNative]::SendMessage($list,0x188,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32() -eq 0) 'Refresh lost stable window selection'
 $root.Text='Kokoda Picker Public';[Windows.Forms.Application]::DoEvents()
 Wait-For {$line=New-Object Text.StringBuilder 4096;[HostGuiNative]::ReadText($list,0x189,[IntPtr]::Zero,$line)|Out-Null;!$line.ToString().Contains('renamed')} 'Picker did not refresh restored title'
 # Capture only this filtered, synthetic preview.
 $pr=New-Object HostGuiNative+Rect;[HostGuiNative]::GetWindowRect($picker,[ref]$pr)|Out-Null
 $pb=New-Object Drawing.Bitmap ($pr.R-$pr.L),($pr.B-$pr.T);$pg=[Drawing.Graphics]::FromImage($pb);$pd=$pg.GetHdc()
 try {[HostGuiNative]::PrintWindow($picker,$pd,2)|Out-Null} finally {$pg.ReleaseHdc($pd);$pg.Dispose()}
 $pb.Save((Join-Path $ResultDirectory 'host-picker.png'),[Drawing.Imaging.ImageFormat]::Png);$pb.Dispose()

 [HostGuiNative]::SendMessage([HostGuiNative]::GetDlgItem($picker,305),0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
 Wait-For {![HostGuiNative]::IsWindow($picker)} 'Picker did not accept rule'
 Wait-For {Test-Path "$data\settings.json"} 'Picker rule was not saved'
 $saved=Get-Content "$data\settings.json" -Raw|ConvertFrom-Json
 Assert ($saved.policy.rules.Count -eq 1 -and $saved.policy.rules[0].title_contains -eq 'Kokoda Picker Public' -and $saved.policy.rules[0].executable) 'Example did not produce combined rule'
 [HostGuiNative]::SendMessage((Control 111),0x186,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null;Click 110
 foreach($f in $fixtures){$f.Close();$f.Dispose()};$fixtures=@()
 Assert (![HostGuiNative]::SendMessage((Control 119),0xF0,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32()) 'Discovery must be opt-in'
 Click 119
 Click 101
 Assert ([HostGuiNative]::IsWindowEnabled((Control 104))) 'Host started without a password'
 $script:password='GUI-test-'+[Guid]::NewGuid().ToString('N')
 Set-Password $script:password
 Assert (!(Test-Path "$data\identity") -and !(Test-Path "$data\pairing")) 'GUI created legacy pairing files'
 Assert (!(Get-Content "$data\settings.json" -Raw).Contains($script:password)) 'Password stored in settings'
 Set-Field 105 '0'; Click 101
 Assert ([HostGuiNative]::IsWindowEnabled((Control 104))) 'Invalid port started service'
 Set-Field 105 "$port";Click 101
 Wait-For {(Get-Field 114) -match 'could not start|共享启动失败'} 'Bind failure was not shown in GUI'
 $listener.Stop();$listener=$null
 Click 101;Wait-Running
 $catalog=List-Windows
 Assert ($catalog.read_only -eq $true -and $catalog.windows.Count -eq 0) 'Default GUI mode exposed windows'
 $null=List-Windows -Password 'Incorrect synthetic password' -Reject $true
 Click 102;Wait-Stopped $workerId
 # A title rule targeting this synthetic settings window exercises the actual preview and host filter.
 Set-Field 107 'Kokoda';Click 109;Click 112
 Assert ([HostGuiNative]::SendMessage((Control 113),0x18B,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32() -ge 1) 'Preview is empty'
 Click 101;Wait-Running
 $catalog=List-Windows
 Assert ($catalog.read_only -and @($catalog.windows | Where-Object {$_.title -notlike '*Kokoda*'}).Count -eq 0) 'GUI rule was not applied'
 Click 102;Wait-Stopped $workerId
 # Capture only the test GUI, never the user's desktop.
 $rect=New-Object HostGuiNative+Rect;[HostGuiNative]::GetWindowRect($window,[ref]$rect)|Out-Null
 $bitmap=New-Object Drawing.Bitmap ($rect.R-$rect.L),($rect.B-$rect.T)
 $graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
 try {[HostGuiNative]::PrintWindow($window,$dc,2)|Out-Null} finally {$graphics.ReleaseHdc($dc);$graphics.Dispose()}
 $bitmap.Save((Join-Path $ResultDirectory 'host-gui.png'),[Drawing.Imaging.ImageFormat]::Png);$bitmap.Dispose()
 [HostGuiNative]::SendMessage((Control 104),0x14E,[IntPtr]1,[IntPtr]::Zero)|Out-Null
 [HostGuiNative]::SendMessage($window,0x111,[IntPtr](104 -bor (1 -shl 16)),[IntPtr]::Zero)|Out-Null
 Click 101;Wait-Running
 # Full-control mode is checked through persisted selection and capability, without logging private titles.
 $catalog=List-Windows;Assert (!$catalog.read_only) 'Explicit all-window mode remains read-only';$catalog=$null
 [HostGuiNative]::PostMessage($window,0x10,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
 Assert ($gui.WaitForExit(10000)) 'GUI did not close';Wait-Stopped $workerId
 Open-Gui
 $old=$script:password;$script:password='Rotated-test-'+[Guid]::NewGuid().ToString('N');Set-Password $script:password
 Assert ((Get-Content "$data\settings.json" -Raw | ConvertFrom-Json).all_windows) 'Settings did not persist'
 Click 101;Wait-Running
 $null=List-Windows -Password $old -Reject $true
 $null=List-Windows
 $gui.Kill();$gui.WaitForExit();Wait-Stopped $workerId
 'PASS: automatic window picker, owner preview, combined rule, discovery opt-in, GUI subsystem, accessible buttons, invalid port, bind failure, password setup/rotation, wrong-password rejection, OPAQUE/TLS, no pairing files, empty default, rules/preview, full mode, restart, close and crash cleanup' | Set-Content -Encoding UTF8 "$ResultDirectory\result.txt"
} catch { if($gui){[HostGuiNative]::TestWindows($gui.Id) | Set-Content -Encoding UTF8 "$ResultDirectory\test-windows.txt"}; $_.ToString() | Set-Content -Encoding UTF8 "$ResultDirectory\failure.txt"; throw }
finally {
 $env:COMPUTERNAME=$originalComputerName
 foreach($f in $fixtures){$f.Close();$f.Dispose()}
 if($listener){$listener.Stop()}
 if($bridge -and !$bridge.HasExited){$bridge.Kill();$bridge.WaitForExit()}
 if($gui -and !$gui.HasExited){$gui.Kill();$gui.WaitForExit()}
 if($workerId){Wait-Stopped $workerId}
 if(Test-Path $data){Remove-Item $data -Recurse -Force}
 Remove-Item Env:\KOKODA_HOST_DATA_DIR -ErrorAction SilentlyContinue
}
