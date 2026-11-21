param([Parameter(Mandatory=$true)][string]$StateFile,[Parameter(Mandatory=$true)][string]$CommandFile,[ValidateSet('Root','Child','Unrelated')][string]$Mode='Root')
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @'
using System;using System.IO;using System.Drawing;using System.Diagnostics;using System.Runtime.InteropServices;using System.Windows.Forms;
public class RolesForm:Form {
 public bool ForceTaskbar;
 protected override CreateParams CreateParams {get {var p=base.CreateParams;if(ForceTaskbar)p.ExStyle|=0x40000;return p;}}
 public RolesForm(string title) {Text=title;AutoScaleMode=AutoScaleMode.None;ClientSize=new Size(500,300);MinimumSize=new Size(280,180);StartPosition=FormStartPosition.Manual;Location=new Point(70,70);}
}
public static class RolesFixture {
 [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr value);
 [DllImport("user32.dll")] static extern bool GetCursorPos(out Point point);
 static string state,command,script,last="";static Timer timer,animation;static int shade;static Process child;
 static TextBox editor;static RolesForm root,independent;static ApplicationContext context;
 static RolesForm Make(string name,int x,int y){var f=new RolesForm(name);f.Location=new Point(x,y);return f;}
 static void WriteState(){Point p;GetCursorPos(out p);int pid=child!=null&&!child.HasExited?child.Id:0;try{File.WriteAllText(state,"{\"fixture_pid\":"+Process.GetCurrentProcess().Id+",\"child_pid\":"+pid+",\"command_base64\":\""+Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes(last))+"\",\"editor_base64\":\""+Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes(editor==null?"":editor.Text))+"\",\"cursor_x\":"+p.X+",\"cursor_y\":"+p.Y+"}");}catch(IOException){}}
 static void Poll(){
  WriteState();string text="";try{if(File.Exists(command))text=File.ReadAllText(command).Trim();}catch(IOException){}
  if(text==last)return;last=text;
  if(text.EndsWith(":animate")&&animation==null){animation=new Timer();animation.Interval=150;animation.Tick+=(s,e)=>{shade=(shade+1)%32;foreach(Form f in Application.OpenForms)f.BackColor=Color.FromArgb(200+shade,220,235);};animation.Start();}
  if(text.EndsWith(":focus-editor")&&editor!=null){root.Activate();editor.Focus();}
  if(text.EndsWith(":clear-editor")&&editor!=null){editor.Clear();root.Activate();editor.Focus();}
  if(text.EndsWith(":copy-text")){Clipboard.SetText("Kokoda clipboard \u4e2d\u6587");}
  if(text.EndsWith(":copy-image")){using(var bitmap=new Bitmap(32,24)){using(var g=Graphics.FromImage(bitmap))g.Clear(Color.FromArgb(33,177,199));Clipboard.SetImage(bitmap);}}
  if(text.EndsWith(":modal")&&!root.IsDisposed){var modal=Make("Kokoda Roles Modal",350,200);modal.ShowInTaskbar=false;modal.FormBorderStyle=FormBorderStyle.FixedDialog;modal.MaximizeBox=false;modal.MinimizeBox=false;modal.ShowDialog(root);modal.Dispose();}
  if(text.EndsWith(":menu")&&!root.IsDisposed){using(var menu=new ContextMenu()){menu.MenuItems.Add(new MenuItem("Synthetic item"));menu.Show(root,new Point(20,20));}}
  if(text.EndsWith(":child")&&child==null){var p=new ProcessStartInfo(Process.GetCurrentProcess().MainModule.FileName,"-NoProfile -ExecutionPolicy Bypass -File \""+script+"\" -Mode Child -StateFile \""+state+"\" -CommandFile \""+command+"\"");p.UseShellExecute=false;p.CreateNoWindow=true;child=Process.Start(p);}
  if(text.EndsWith(":stop")){if(child!=null&&!child.HasExited)child.Kill();Application.ExitThread();}
 }
 public static void Run(string mode,string stateFile,string commandFile,string scriptFile){
  SetProcessDpiAwarenessContext(new IntPtr(-4));
  if(mode!="Root"){var f=Make("Kokoda Roles "+(mode=="Child"?"Child Process":"Unrelated Process"),900,500);Application.Run(f);return;}
  state=stateFile;command=commandFile;script=scriptFile;
  context=new ApplicationContext();root=Make("Kokoda Roles Root",70,70);editor=new TextBox();editor.Multiline=true;editor.SetBounds(20,20,400,150);editor.Font=new Font("Segoe UI",16);root.Controls.Add(editor);root.Show();
  independent=Make("Kokoda Roles Independent",750,80);independent.Show();
  var panel=Make("Kokoda Roles Tool",650,450);panel.FormBorderStyle=FormBorderStyle.SizableToolWindow;panel.ShowInTaskbar=false;panel.Show(root);
  var owned=Make("Kokoda Roles Owned App",350,450);owned.ForceTaskbar=true;owned.Show(root);
  var fixedDialog=Make("Kokoda Roles Fixed",80,500);fixedDialog.ShowInTaskbar=false;fixedDialog.FormBorderStyle=FormBorderStyle.FixedDialog;fixedDialog.MaximizeBox=false;fixedDialog.MinimizeBox=false;fixedDialog.Show(root);
  timer=new Timer();timer.Interval=100;timer.Tick+=(s,e)=>Poll();timer.Start();WriteState();
  try{Application.Run(context);}finally{timer.Stop();timer.Dispose();if(animation!=null){animation.Stop();animation.Dispose();}if(child!=null&&!child.HasExited)child.Kill();}
 }
}
'@
[RolesFixture]::Run($Mode,$StateFile,$CommandFile,$PSCommandPath)
