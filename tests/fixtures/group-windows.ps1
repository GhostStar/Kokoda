param([string]$ResultFile = "$env:TEMP\Kokoda-group-input.txt")
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @'
using System;
using System.IO;
using System.Drawing;
using System.Windows.Forms;
using System.Runtime.InteropServices;
public class GroupFixture : Form {
 [DllImport("user32.dll",EntryPoint="SetWindowLongPtrW")] static extern IntPtr SetWindowLongPtr(IntPtr h,int n,IntPtr value);
 public static string ResultPath;
 Form card;
 public GroupFixture() {
  Text="Kokoda Group Test"; ClientSize=new Size(600,360); StartPosition=FormStartPosition.Manual;Location=new Point(80,80);
  var show=new Button{Text="Show card",Location=new Point(20,40),Size=new Size(180,80)};
  show.Click+=(s,e)=>ShowCard();Controls.Add(show);
  var hide=new Button{Text="Hide card",Location=new Point(240,40),Size=new Size(180,80)};
  hide.Click+=(s,e)=>{if(card!=null)card.Hide();};Controls.Add(hide);
  var modal=new Button{Text="Modal dialog",Location=new Point(440,40),Size=new Size(140,80)};
  modal.Click+=(s,e)=>{
   using(var dialog=new Form{Text="Kokoda Modal Dialog",ClientSize=new Size(280,160),StartPosition=FormStartPosition.CenterParent}) {
    var done=new Button{Text="Close",Dock=DockStyle.Bottom,DialogResult=DialogResult.Cancel};
    dialog.Controls.Add(done);dialog.CancelButton=done;dialog.ShowDialog(this);
   }
  };Controls.Add(modal);
  var close=new Button{Text="Destroy card",Location=new Point(20,180),Size=new Size(180,80)};
  close.Click+=(s,e)=>{if(card!=null)card.Close();};Controls.Add(close);
 }
 void ShowCard() {
  if(card==null||card.IsDisposed) {
   card=new Form{Text="Kokoda Group Card",ClientSize=new Size(280,180),StartPosition=FormStartPosition.Manual,Location=new Point(720,120)};
   var text=new TextBox{Multiline=true,Dock=DockStyle.Fill};
   text.TextChanged+=(s,e)=>File.WriteAllText(ResultPath,text.Text);
   card.Controls.Add(text);
  }
  card.Show();SetWindowLongPtr(card.Handle,-8,IntPtr.Zero);card.Activate();
 }
 public static void Run(){Application.Run(new GroupFixture());}
}
'@
[GroupFixture]::ResultPath=$ResultFile
[GroupFixture]::Run()
