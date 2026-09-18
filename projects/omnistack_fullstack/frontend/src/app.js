// OmniStack Cloud Frontend Controller
document.addEventListener("DOMContentLoaded", () => {
  const themeToggle = document.getElementById("theme-toggle");
  themeToggle?.addEventListener("click", () => {
    document.body.classList.toggle("theme-light");
  });

  const navItems = document.querySelectorAll(".nav-item");
  const title = document.getElementById("page-title");
  
  navItems.forEach(item => {
    item.addEventListener("click", () => {
      navItems.forEach(n => n.classList.remove("active"));
      item.classList.add("active");
      const tab = item.dataset.tab;
      if (title) title.innerText = item.innerText;
      if (window.renderTabContent) {
        window.renderTabContent(tab);
      }
    });
  });

  console.log("[OmniStack] Initialized client application under Draft VCS.");
});
