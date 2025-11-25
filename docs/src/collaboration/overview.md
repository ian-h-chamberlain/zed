# Collaboration

At Zed, we believe that great things are built by great people working together.
We have designed Zed to help individuals work faster and help teams of people work together more effectively.

In Zed, all collaboration happens in the collaboration panel, which can be opened via {#kb collab_panel::ToggleFocus} or `collab panel: toggle focus` from the command palette.
You will need to [sign in](../../authentication.md#signing-in) in order to access features within the collaboration panel.

Once signed in, you'll notice the collaboration panel is broken down into two categories:

1. [Channels](./channels.md): Ongoing rooms where team members can share projects, collaborate on code, and maintain ambient awareness of what everyone is working on.
1. [Contacts](./contacts.md): Your list of personal contacts with whom you can start ad-hoc private collaboration sessions.

Whether you're collaborating in channels or privately with contacts, the core experience is quite similar.
Let's explore the key differences and then dive into the shared features.

## Channels

### Overview

Channels provide a way to streamline collaborating for software engineers in many ways, but particularly:

- Pairing – when working on something together, you both have your own screen, mouse, and keyboard.
- Mentoring – it’s easy to jump in to someone else’s context, and help them get unstuck, without the friction of pushing code up.
- Refactoring – you can have multiple people join in on large refactoring without fear of conflict.
- Ambient awareness – you can see what everyone else is working on with no need for status emails or meetings.

### Channels

Each channel corresponds to an ongoing project or work-stream.
You can see who’s in a channel as their avatars will show up in the sidebar.
This makes it easy to see what everyone is doing and where to find them if needed.

You can create as many channels as you need.
You can mix channels for your day job, as well as side-projects in one instance of Zed.

Joining a channel adds you to a shared room where you can work on projects together.

### Channel Notes

Each channel has a notes file associated with it to keep track of current status, new ideas, or to collaborate on building out the design for the feature that you’re working on before diving into code.

This is similar to a Google Doc, except powered by Zed's collaborative software and persisted to our servers.

### Sharing projects

After joining a channel, you can `Share` a project with the other people there. This will enable them to edit the code hosted on your machine as though they had it checked out locally.

When you are editing someone else’s project, you still have the full power of the editor at your fingertips, you can jump to definitions, use the AI assistant, and see any diagnostic errors. This is extremely powerful for pairing, as one of you can be implementing the current method while the other is reading and researching the correct solution to the next problem. And, because you have your own config running, it feels like you’re using your own machine.

### Livestreaming & Guests

A Channel can also be made Public. This allows anyone to join the channel by clicking on the link.

Guest users in channels can hear and see everything that is happening, and have read only access to projects and channel notes.

If you'd like to invite a guest to participate in a channel for the duration of a call you can do so by right clicking on them in the Collaboration Panel. "Allowing Write Access" will allow them to edit any projects shared into the call, and to use their microphone and share their screen if they wish.

### Inviting people

By default, channels you create can only be accessed by you. You can invite collaborators by right clicking and selecting `Manage members`.

When you have channels nested under each other, permissions are inherited. For instance, in the example above, we only need to add people to the `#zed` channel, and they will automatically gain access to `#core-editor`, `#new-languages`, and `#stability`.

Once you have added someone, they can either join your channel by clicking on it in their Zed sidebar, or you can share the link to the channel so that they can join directly.

## Contacts

Zed allows you to add contacts.
Similar to channels, you can have private calls / collaboration sessions with those in your contacts.
These calls can be one-on-ones or contain any number of users from your contacts.

### Adding a contact

1. In the collaboration panel, click the `+` button to the right of the `Contacts` section to `Search for new contact`.
1. Search for the contact you want to add using their GitHub handle.
   _Note: The contact you're adding must be an existing Zed user who has completed the GitHub authentication sign-in flow._
1. Your contact will receive a notification of your request.
   Once they accept, you'll both appear in each other's `Contacts` list.

## Collaboration features

### Voice chat

When joining a channel, Zed will automatically share your microphone, if your OS allows it. This isn't tied to your project. You can disable this for your client via the [`mute_on_join`](../configuring-zed.md#calls) setting.

## Collaborating on a project

### Share a project

When you invite a collaborator to join your project, a new call begins. Your Zed windows will show the call participants in the title bar of the window.

![A new Zed call with two collaborators](https://zed.dev/img/collaboration/new-call.png)

Collaborators in the same project as you are in color, and have a cursor color. Collaborators in other projects are shown in gray. Collaborators that have access to the current project will have their own cursor color under their avatar.

We aim to eliminate the distinction between local and remote projects as much as possible. Collaborators can open, edit, and save files, perform searches, interact with the language server, etc. Guests have a read-only view of the project, including access to language server info.

#### Unshared Projects

If a collaborator is currently in a project that is not shared, you will not be able to jump to their project or follow them until they either share the project or return to a project that is shared.

If you are in a project that isn't shared, others will not be able to join it or see its contents.

### Follow a collaborator

To follow a collaborator, click on their avatar in the top right of the window. You can also cycle through collaborators using `workspace: follow next collaborator` (`ctrl-alt-cmd-f`).

When you join a project, you'll immediately start following the collaborator that invited you.

![Automatically following the person inviting us to a project](https://zed.dev/img/collaboration/joining-a-call.png)

When you are in a pane that is following a collaborator, you will:

- follow their cursor and scroll position
- follow them to other files in the same project
- instantly swap to viewing their screen in that pane, if they are sharing their screen and leave the project

If you move your cursor or make an edit in that pane, you will stop following.

To start following again, you can click on a collaborator's avatar or cycle through following different participants by pressing `workspace: follow next collaborator` (`ctrl-alt-cmd-f`).

#### How following works

Following is confined to a particular pane. When a pane is following a collaborator, it is outlined in their cursor color.

This pane-specific behavior allows you to follow someone in one pane while navigating independently in another and can be an effective layout for some collaboration styles.

### Sharing your screen

Share your screen with collaborators in the current call by clicking on the `Share screen` button in the top right of the window.

Collaborators will see your screen if they are following you and you start viewing a window outside Zed or a project that is not shared.

Collaborators can see your entire screen when you are screen sharing, so be careful not to share anything you don't want to share. Remember to stop screen sharing when you are finished.

Call participants can open a dedicated tab for your screen share by opening the contacts menu in the top right and clicking on the `Screen` entry if you are sharing your screen.

### Adding a project

You can add a project to a call by clicking on the `Share` button next to the project name in the title bar.

### Removing a project

You can remove a project from a call by clicking on the `Unshare` button next to the project name in the title bar.

Collaborators that are currently in that project will be disconnected from the project and will not be able to rejoin it unless you share it again.

### Following a collaborator's terminal

You can follow what a collaborator is doing in their terminal by having them share their screen and following it.

In the future, we plan to allow you to collaborate in the terminal directly in a shared project.

### Leave call

You can leave a call by opening the contacts menu in the top right and clicking on the `Leave call` button.


---

> Note: Only collaborate with people that you trust.
> Since sharing a project gives them access to your local file system, you should not share projects with people you do not trust; they could potentially do some nasty things.
>
> In the future, we will do more to prevent this type of access beyond the shared project and add more control over what collaborators can do, but for now, only collaborate with people you trust.
