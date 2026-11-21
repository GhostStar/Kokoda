param([string]$ResultFile = "$env:TEMP\Kokoda-clipboard-input.txt", [string]$WindowTitle = "Kokoda Clipboard Test")
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @'
using System;
using System.IO;
using System.Windows.Forms;
using System.Drawing;
public class ClipboardFixture : Form {
 public static string ResultPath;
 public static string WindowTitle;
 DataObject backup = new DataObject(); bool empty;
 public ClipboardFixture() {
  Text=WindowTitle;ClientSize=new Size(600,400);
  var original=Clipboard.GetDataObject();empty=original==null;
  if(original!=null)foreach(var format in original.GetFormats(false)) {
   var data=original.GetData(format,false);if(data!=null)backup.SetData(format,false,data);
  }
  // Virtual-key assertions must not be consumed by the host IME. Unicode commits are tested separately.
  var text=new TextBox{ImeMode=ImeMode.Disable,Multiline=true,AcceptsTab=true,Dock=DockStyle.Fill,Font=new Font("Segoe UI",16)};
  text.KeyDown+=(s,e)=>{if(e.Control && e.KeyCode==Keys.A){text.SelectAll();e.SuppressKeyPress=true;}
   if(e.KeyCode==Keys.F8){text.Copy();e.SuppressKeyPress=true;}
  };
  text.TextChanged+=(s,e)=>File.WriteAllText(ResultPath,text.Text);
  File.WriteAllText(ResultPath,"");Controls.Add(text);Shown+=(s,e)=>text.Focus();
  FormClosing+=(s,e)=>{
   if(Clipboard.ContainsText() && Clipboard.GetText().StartsWith("Kokoda clipboard")) {
    if(empty)Clipboard.Clear();else Clipboard.SetDataObject(backup,true);
   }
  };
 }
 public static void Run(){Application.Run(new ClipboardFixture());}
}
'@
[ClipboardFixture]::ResultPath=$ResultFile
[ClipboardFixture]::WindowTitle=$WindowTitle
[ClipboardFixture]::Run()
